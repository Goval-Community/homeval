use axum::extract::{State, Path, ConnectInfo};
use axum::extract::ws::WebSocket;
use axum::response::Response;
use axum::routing::get;
use axum::{Router, extract::ws::{WebSocketUpgrade, Message as WsMessage}};
#[cfg(feature = "fun-stuff")]
use chrono::Datelike;

use deno_core::error::AnyError;
use homeval::goval;
use homeval::goval::Command;
use prost::Message;
use tokio::sync::mpsc::UnboundedSender;
use std::net::SocketAddr;
use std::{env, sync::Arc};

use futures_util::{SinkExt, StreamExt};
use log::{error, info, trace, warn, debug};
use tokio::{
    sync::mpsc,
};

use crate::{
    MAX_CHANNEL,
    CHANNEL_METADATA,
    CHANNEL_MESSAGES,
    PROCCESS_CHANNEL_TO_ID,
    SESSION_CHANNELS,
    SESSION_CLIENT_INFO,
    LAST_SESSION_USING_CHANNEL,
    IMPLEMENTED_SERVICES,
    SESSION_MAP,
    MAX_SESSION, 
};

use crate::{
    channels::IPCMessage,
    deno_extension::{
        make_extension,
        JsMessage,
        Service
    },
    parse_paseto::{
        parse,
        ClientInfo
    },
    services
};

#[derive(Clone)]
struct AppState {
    sender: UnboundedSender<IPCMessage>
}

pub async fn start_server() -> Result<(), AnyError> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    
    let (tx, mut rx) = mpsc::unbounded_channel::<IPCMessage>();

    let app = Router::new()
        .route("/wsv2/:token", get(wsv2))
        .route("/", get(default_handler))
        .with_state(AppState { sender: tx });
    info!("Goval server listening on: {}", addr);

    tokio::spawn(async move {
        axum::Server::bind(&addr.parse().unwrap())
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await.unwrap()
    });

    while let Some(message) = rx.recv().await {
        handle_message(message, SESSION_MAP.clone()).await;
    }

    Ok(())
}

async fn default_handler() -> String {
    "(づ ◕‿◕ )づ Hello there".to_string()
}

async fn on_wsv2_upgrade(socket: WebSocket, token: String, state: AppState, addr: SocketAddr) {
    debug!("Waiting for mutex...");
    let mut max_session = MAX_SESSION.lock().await;

    debug!("Mutex acquired...");
    *max_session += 1;
    let session_id = max_session.clone();
    drop(max_session);

    let (send_to_session, session_recv) = mpsc::unbounded_channel::<IPCMessage>();
    SESSION_MAP
        .write()
        .await
        .insert(session_id, send_to_session);

    let tx_clone = state.sender.clone();
    match accept_connection(
        socket,
        tx_clone,
        session_recv,
        session_id,
        parse(&token).unwrap_or(ClientInfo::default()),
        addr
    ).await {
        Ok(_) => {}
        Err(err) => {
            error!("accept_connection errored: {}", err)
        }
    };
    
}

async fn wsv2(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>
) -> Response {
    ws.on_upgrade(move |socket| on_wsv2_upgrade(socket, token, state, addr))
}

async fn handle_message(message: IPCMessage, session_map: Arc<tokio::sync::RwLock<std::collections::HashMap<i32, mpsc::UnboundedSender<IPCMessage>>>>) {
    let cmd: Command;
        match message.to_cmd() {
            Err(err) => {
                error!(
                    "Following error occured when decoding message in main loop: {}",
                    err
                );
                return;
            }
            Ok(res) => cmd = res,
        }

        let cmd_body: goval::command::Body;

        match cmd.body {
            None => {
                error!("MISSING COMMAND BODY: {:#?}", cmd);
                return;
            }
            Some(body) => cmd_body = body,
        }

        if cmd.channel == 0 {
            match cmd_body {
                goval::command::Body::Ping(_) => {
                    let mut pong = goval::Command::default();
                    pong.body = Some(goval::command::Body::Pong(goval::Pong::default()));
                    pong.r#ref = cmd.r#ref;
                    pong.channel = 0;

                    if let Some(sender) = session_map.read().await.get(&message.session) {
                        match sender.send(message.replace_cmd(pong)) {
                            Ok(_) => {}
                            Err(err) => {
                                error!("Error occured while sending Pong {}", err);
                            }
                        }
                    } else {
                        error!("Missing session queue when sending Pong")
                    }
                }
                goval::command::Body::OpenChan(open_chan) => {
                    if IMPLEMENTED_SERVICES.contains(&open_chan.service) {
                        let mut found = false;
                        let mut channel_id_held = 0;

                        let attach = open_chan.action()
                            == goval::open_channel::Action::AttachOrCreate
                            || open_chan.action() == goval::open_channel::Action::Attach
                            || open_chan.service == "git"; // git is just use for replspace api stuff
                                                           // from what I can tell, so its just easier to have it as one instance.
                        let create = open_chan.action()
                            == goval::open_channel::Action::AttachOrCreate
                            || open_chan.action() == goval::open_channel::Action::Create;
                        if attach {
                            let metadata = CHANNEL_METADATA.read().await;
                            for channel in metadata.iter() {
                                if channel.name.is_some()
                                    && channel.name.clone().unwrap_or("".to_string())
                                        == open_chan.name
                                    && channel.service.clone() == open_chan.service
                                {
                                    found = true;
                                    channel_id_held = channel.id.clone();
                                    continue;
                                }
                            }
                        }

                        if !found && create {
                            trace!("executing openchan main block");
                            let service = open_chan.service.clone();
                            let mut max_channel = MAX_CHANNEL.lock().await;
                            *max_channel += 1;
                            let channel_id = max_channel.clone();
                            channel_id_held = channel_id.clone();
                            drop(max_channel);

                            let _channel_name: Option<String>;

                            if open_chan.name.len() > 0 {
                                _channel_name = Some(open_chan.name);
                            } else {
                                _channel_name = None;
                            }

                            let service_data = Service {
                                service: service.clone(),
                                id: channel_id,
                                name: _channel_name,
                            };

                            trace!("Awaiting queue write for channel {}", channel_id);

                            CHANNEL_MESSAGES.write().await.insert(
                                channel_id_held,
                                Arc::new(deadqueue::unlimited::Queue::new()),
                            );
                            let mut metadata = CHANNEL_METADATA.write().await;
                            metadata.push(service_data.clone());
                            drop(metadata);
                            trace!("Added channel: {} to queue list", channel_id_held);

                            tokio::task::spawn_blocking(move || {
                                let local = tokio::task::LocalSet::new();

                                let rt = tokio::runtime::Handle::current();
                                rt.block_on(async {
                                    local
                                        .run_until(async {
                                            let mod_path =
                                                &format!("services/{}.js", open_chan.service);
                                            trace!(
                                                "Loading module path: {} for service: {}",
                                                mod_path,
                                                open_chan.service
                                            );
                                            let main_module: deno_core::url::Url;
                                            let main_module_res = deno_core::resolve_path(
                                                mod_path,
                                                std::env::current_dir().unwrap().as_path(),
                                            );
                                            match main_module_res {
                                                Err(err) => {
                                                    error!("Error resolving js module {}", err);
                                                    return;
                                                }
                                                Ok(result) => {
                                                    main_module = result;
                                                }
                                            }

                                            let mut js_runtime = deno_core::JsRuntime::new(
                                                deno_core::RuntimeOptions {
                                                    module_loader: Some(std::rc::Rc::new(
                                                        deno_core::FsModuleLoader,
                                                    )),
                                                    extensions: vec![make_extension()],
                                                    ..Default::default()
                                                },
                                            );

                                            js_runtime
                                                .execute_script(
                                                    "[goval::generated::globals]",
                                                    format!(
                                                        "globalThis.serviceInfo = {};",
                                                        serde_json::to_string(&service_data)
                                                            .unwrap()
                                                    )
                                                    .into(),
                                                )
                                                .unwrap();
                                            js_runtime
                                                .execute_script(
                                                    "[goval::runtime.js]",
                                                    deno_core::FastString::ensure_static_ascii(
                                                        include_str!("./runtime.js"),
                                                    ),
                                                )
                                                .unwrap();
                                            js_runtime
                                                .execute_script(
                                                    "[goval::api.js]",
                                                    deno_core::FastString::ensure_static_ascii(
                                                        include_str!(concat!(
                                                            env!("OUT_DIR"),
                                                            "/api.js"
                                                        )),
                                                    ),
                                                )
                                                .unwrap();

                                            let mod_id = js_runtime
                                                .load_main_module(
                                                    &main_module,
                                                    services::get_module_core(open_chan.service.clone())
                                                        .expect("Error fetching module code"),
                                                )
                                                .await
                                                .unwrap();
                                            let result = js_runtime.mod_evaluate(mod_id);

                                            js_runtime.run_event_loop(false).await.unwrap();
                                            
                                            match result.await {
                                                Ok(inner) => {
                                                    match inner {
                                                        Ok(_) => {},
                                                        Err(err) => {
                                                            error!("Got error in v8 thread for service {}:\n{}", open_chan.service, err)
                                                        }
                                                    }
                                                },
                                                Err(err) => {
                                                    error!("Got the following canceled error in v8 thread for service {}: {}", open_chan.service, err)
                                                }
                                            }
                                        })
                                        .await;
                                });
                            });
                            found = true;
                        }

                        if !found {
                            error!("Couldnt make channel");
                            let mut protocol_error = goval::Command::default();
                            let mut _inner = goval::ProtocolError::default();
                            _inner.text = "Could not create / attach channel".to_string();

                            protocol_error.body = Some(goval::command::Body::ProtocolError(_inner));
                            protocol_error.r#ref = cmd.r#ref;
                            protocol_error.channel = 0;

                            session_map
                                .read()
                                .await
                                .get(&message.session)
                                .unwrap()
                                .send(message.replace_cmd(protocol_error))
                                .unwrap();
                            return;
                        }

                        let mut open_chan_res = goval::Command::default();
                        let mut _open_res = goval::OpenChannelRes::default();
                        _open_res.state = goval::open_channel_res::State::Created.into();
                        _open_res.id = channel_id_held;
                        open_chan_res.body = Some(goval::command::Body::OpenChanRes(_open_res));
                        open_chan_res.r#ref = cmd.r#ref;
                        open_chan_res.channel = 0;

                        session_map
                            .read()
                            .await
                            .get(&message.session)
                            .unwrap()
                            .send(message.replace_cmd(open_chan_res))
                            .unwrap();

                        let msg_read = CHANNEL_MESSAGES.read().await;

                        let queue = msg_read.get(&channel_id_held).unwrap().clone();

                        drop(msg_read);

                        queue.push(JsMessage::Attach(message.session));

                        SESSION_CHANNELS
                            .write()
                            .await
                            .entry(message.session)
                            .and_modify(|channels| channels.push(channel_id_held));
                    } else {
                        warn!(
                            "Missing service requested by openChan: {}",
                            open_chan.service
                        )
                    }
                }
                _ => {}
            }
        } else {
            // Directly deal with Command::Input, should be faster
            if let goval::command::Body::Input(input) = cmd_body {
                if let Some(pty_id) = PROCCESS_CHANNEL_TO_ID.read().await.get(&cmd.channel) {
                    let mut to_continue = false;
                    if let Some(queue) = crate::PROCCESS_WRITE_MESSAGES.read().await.get(&pty_id) {
                        queue.push(input);
                        to_continue = true;
                    } else {
                        error!("Couldn't find pty {} to write to", pty_id);
                    }

                    if to_continue {
                        return;
                    }
                }
            }

            let msg_lock = CHANNEL_MESSAGES.read().await;

            let queue = msg_lock.get(&cmd.channel).unwrap().clone();

            drop(msg_lock);

            queue.push(JsMessage::IPC(message.clone()));

            let mut hashmap_lock = LAST_SESSION_USING_CHANNEL.write().await;

            hashmap_lock.insert(cmd.channel, message.session);

            drop(hashmap_lock);
        }
}

async fn send_message(
    message: goval::Command,
    stream: &mut futures_util::stream::SplitSink<
        WebSocket,
        WsMessage,
    >,
) -> Result<(), AnyError> {
    let mut buf = Vec::new();
    buf.reserve(message.encoded_len());
    message.encode(&mut buf)?;

    Ok(stream.send(WsMessage::Binary(buf)).await?)
}

async fn accept_connection(
    ws_stream: WebSocket,
    propagate: mpsc::UnboundedSender<IPCMessage>,
    mut sent: mpsc::UnboundedReceiver<IPCMessage>,
    session: i32,
    client: ClientInfo,
    addr: SocketAddr
) -> Result<(), AnyError> {
    info!("New connection with peer address: {}", addr);

    info!("New client: {:#?}", client);

    SESSION_CLIENT_INFO
        .write()
        .await
        .insert(session, client.clone());

    let (mut write, mut read) = ws_stream.split();

    let mut boot_status = goval::Command::default();
    let mut inner = goval::BootStatus::default();
    inner.stage = goval::boot_status::Stage::Complete.into();
    boot_status.body = Some(goval::command::Body::BootStatus(inner));

    send_message(boot_status, &mut write).await?;

    // Sending container state
    let mut container_state = goval::Command::default();
    let mut inner_state = goval::ContainerState::default();
    inner_state.state = goval::container_state::State::Ready.into();
    container_state.body = Some(goval::command::Body::ContainerState(inner_state));

    send_message(container_state, &mut write).await?;

    // Sending server info message
    let mut toast = goval::Command::default();
    let mut inner_state = goval::Toast::default();
    inner_state.text = format!("Hello @{}, welcome to homeval!", client.username);
    toast.body = Some(goval::command::Body::Toast(inner_state));

    send_message(toast, &mut write).await?;

    #[cfg(feature = "fun-stuff")]
    let date = chrono::Local::now().with_timezone(&chrono_tz::GMT) + chrono::Duration::hours(1);
    #[cfg(feature = "fun-stuff")]
    if date.month() == 6 && date.day() == 10 {
        let mut toast = goval::Command::default();
        let mut inner_state = goval::Toast::default();
        inner_state.text = "Say happy birthday to @haroon!".to_string();
        toast.body = Some(goval::command::Body::Toast(inner_state));

        send_message(toast, &mut write).await?;
    }

    SESSION_CHANNELS.write().await.insert(session, vec![]);
    tokio::spawn(async move {
        while let Some(_msg) = read.next().await {
            match _msg {
                Ok(msg) => match msg {
                    WsMessage::Binary(buf) => {
                        if let Err(err) = propagate.send(IPCMessage {
                            bytes: buf,
                            session,
                        }) {
                            error!("The following error occured when enqueing message to global messagq queue (session: {}): {}", session, err)
                        }
                    }
                    WsMessage::Close(_) => {
                        warn!("CLOSING SESSION {}", session);
                        for _channel in SESSION_CHANNELS.read().await.get(&session).unwrap().iter()
                        {
                            let channel = _channel.clone();
                            tokio::spawn(async move {
                                let msg_lock = CHANNEL_MESSAGES.read().await;

                                let queue = msg_lock.get(&channel).unwrap().clone();

                                drop(msg_lock);

                                queue.push(JsMessage::Close(session));
                            });
                        }

                        SESSION_MAP.write().await.remove(&session);
                        SESSION_CLIENT_INFO.write().await.remove(&session);
                        SESSION_CHANNELS.write().await.remove(&session);
                        warn!("CLOSED SESSION {}", session);
                    }
                    _ => {}
                },
                Err(err) => {
                    error!(
                        "The following error occured while reading messages (session {}): {}",
                        session, err
                    );
                }
            };
        }
    });

    while let Some(i) = sent.recv().await {
        match write.send(WsMessage::Binary(i.bytes)).await {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "The following error occured while sending a message: {}",
                    err
                )
            }
        }
    }

    Ok(())
}

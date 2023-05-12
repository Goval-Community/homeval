use deno_core::error::AnyError;
#[allow(unused_imports)]
use goval_impl::goval;
use goval_impl::goval::Command;
use prost::Message;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response, ErrorResponse};
//use std::fmt::Binary;
use std::{env, io::Error, sync::Arc};

use futures_util::{future, SinkExt, StreamExt, TryStreamExt};
use tokio;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex, RwLock},
};
//use async_log::span;
use log::{error, info, debug, warn};
mod channels;
use channels::IPCMessage;

mod deno_extension;
use deno_extension::{make_extension, JsMessage, Service};

mod parse_paseto;
use parse_paseto::parse;

use lazy_static::lazy_static;

lazy_static! {
    static ref MAX_SESSION: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
    static ref MAX_CHANNEL: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

use c_map::HashMap;
lazy_static! {
    static ref CHANNEL_MESSAGES: HashMap<i32, deadqueue::unlimited::Queue<JsMessage>> =
        HashMap::new();
    static ref CHANNEL_METADATA: RwLock<Vec<Service>> = RwLock::new(vec![]);
    static ref SESSION_MAP: Arc<HashMap<i32, mpsc::UnboundedSender<IPCMessage>>> =
        Arc::new(HashMap::new());
    static ref SESSION_CHANNELS: HashMap<i32, Vec<i32>> = HashMap::new();

    static ref IMPLEMENTED_SERVICES: Vec<String> = vec!["gcsfiles".to_string(), "ot".to_string(), "presence".to_string()];
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // if cfg!(features = "debug") {
    console_subscriber::init();
    // }
    //span!("main", {
    let _ = env_logger::try_init();
    info!("starting...");
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);

    let (tx, mut rx) = mpsc::unbounded_channel::<IPCMessage>();

    let session_map_clone = SESSION_MAP.clone();

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            println!("Waiting for mutex...");
            let mut max_session = MAX_SESSION.lock().await;

            println!("Mutex acquired...");
            *max_session += 1;
            let session_id = max_session.clone();

            let (send_to_session, session_recv) = mpsc::unbounded_channel::<IPCMessage>();
            session_map_clone.write(session_id).insert(send_to_session);

            tokio::spawn(accept_connection(
                stream,
                tx.clone(),
                session_recv,
                session_id,
            ));
        }
    });

    let session_map_clone = SESSION_MAP.clone();
    let _channel_map: RwLock<HashMap<i32, deno_core::JsRuntime>> = RwLock::new(HashMap::new());
    while let Some(message) = rx.recv().await {
        let cmd: Command;
        let command_res = message.to_cmd();
        if let Err(err) = command_res {
            error!("Following error occured when decoding message in main loop: {}", err);
            continue;
        } else {
            cmd = command_res.expect("Impossible condition, decoded message was already verified as not None")
        }

        if cmd.body.is_none() {
            continue;
        }

        if cmd.channel == 0 {
            match cmd.body.expect("This case is impossible, Command#body is none while having been checked to not be none earlier") {
                goval::command::Body::Ping(_) => {
                    let mut pong = goval::Command::default();
                    pong.body = Some(goval::command::Body::Pong(goval::Pong::default()));
                    pong.r#ref = cmd.r#ref;
                    pong.channel = 0;

                    if let Some(sender) = session_map_clone.read(&message.session).get() {
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
                            || open_chan.action() == goval::open_channel::Action::Attach;
                        let create = open_chan.action()
                            == goval::open_channel::Action::AttachOrCreate
                            || open_chan.action() == goval::open_channel::Action::Create;
                        if attach {
                            let metadata = CHANNEL_METADATA.read().await;
                            for channel in metadata.iter() {
                                if channel.name.clone().unwrap_or("".to_string()) == open_chan.name
                                    && channel.service.clone() == open_chan.service
                                {
                                    found = true;
                                    channel_id_held = channel.id.clone();
                                    continue;
                                }
                            }
                        }

                        if !found && create {
                            info!("executing openchan main block");
                            let service = open_chan.service.clone();
                            let mut max_channel = MAX_CHANNEL.lock().await;
                            *max_channel += 1;
                            let channel_id = max_channel.clone();
                            channel_id_held = channel_id.clone();
                            drop(max_channel);

                            let service_data = Service {
                                service: service.clone(),
                                id: channel_id,
                                name: Some(open_chan.name),
                            };

                            info!("Awating queue write");

                            CHANNEL_MESSAGES
                                .write(channel_id_held)
                                .insert(deadqueue::unlimited::Queue::new());
                            let mut metadata = CHANNEL_METADATA.write().await;
                            metadata.push(service_data.clone());
                            drop(metadata);
                            info!("Added channel: {} to queue list", channel_id_held);

                            tokio::task::spawn_blocking(move || {
                                let local = tokio::task::LocalSet::new();

                                let rt = tokio::runtime::Handle::current();
                                rt.block_on(async {
                                    local
                                        .run_until(async {
                                            // local.spawn_local(async move {
                                            let mod_path = &format!("services/{}.js", open_chan.service);
                                            debug!("Module path: {}", mod_path);
                                            let main_module: deno_core::url::Url;
                                            let main_module_res = deno_core::resolve_path(
                                                mod_path
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
                                                    &format!(
                                                        "globalThis.serviceInfo = {};",
                                                        serde_json::to_string(&service_data)
                                                            .unwrap()
                                                    ),
                                                )
                                                .unwrap();
                                            js_runtime
                                                .execute_script(
                                                    "[goval::runtime.js]",
                                                    include_str!("./runtime.js"),
                                                )
                                                .unwrap();
                                            js_runtime
                                                .execute_script(
                                                    "[goval::api.js]",
                                                    include_str!("./api.js"),
                                                )
                                                .unwrap();

                                            let mod_id = js_runtime
                                                .load_main_module(&main_module, None)
                                                .await
                                                .unwrap();
                                            let result = js_runtime.mod_evaluate(mod_id);

                                            js_runtime.run_event_loop(false).await.unwrap();
                                            result.await.unwrap().unwrap();
                                            // })
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

                            session_map_clone
                                .read(&message.session)
                                .get()
                                .unwrap()
                                .send(message.replace_cmd(protocol_error))
                                .unwrap();
                            continue;
                        }

                        let mut open_chan_res = goval::Command::default();
                        let mut _open_res = goval::OpenChannelRes::default();
                        _open_res.state = goval::open_channel_res::State::Created.into();
                        _open_res.id = channel_id_held;
                        open_chan_res.body = Some(goval::command::Body::OpenChanRes(_open_res));
                        open_chan_res.r#ref = cmd.r#ref;
                        open_chan_res.channel = 0;

                        session_map_clone
                            .read(&message.session)
                            .get()
                            .unwrap()
                            .send(message.replace_cmd(open_chan_res))
                            .unwrap();

                        CHANNEL_MESSAGES
                            .read(&channel_id_held)
                            .get()
                            .unwrap()
                            .push(JsMessage::Attach(message.session));
                        
                        SESSION_CHANNELS.write(message.session).entry().and_modify(
                            |channels| { channels.push(channel_id_held) }
                        );
                        // }
                    }
                }
                _ => {}
            }
        } else {
            CHANNEL_MESSAGES
                .read(&cmd.channel)
                .get()
                .unwrap()
                .push(JsMessage::IPC(message));
        }
    }

    Ok(())
}

async fn send_message(
    message: goval::Command,
    stream: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<TcpStream>,
        tungstenite::Message,
    >,
) -> Result<(), AnyError> {
    let mut buf = Vec::new();
    buf.reserve(message.encoded_len());
    message.encode(&mut buf)?;

    Ok(stream.send(tungstenite::Message::Binary(buf)).await?)
}

async fn accept_connection(
    stream: TcpStream,
    propogate: mpsc::UnboundedSender<IPCMessage>,
    mut sent: mpsc::UnboundedReceiver<IPCMessage>,
    session: i32,
) {
    let addr = stream
        .peer_addr()
        .expect("connected streams should have a peer address");
    info!("Peer address: {}", addr);

    let copy_headers_callback = |req: &Request, res: Response| -> Result<Response, ErrorResponse> {
        info!("The request's path is: {}", req.uri().path());

        Ok(res)
    };

    // let mut websocket = accept_hdr(stream.unwrap(), copy_headers_callback).unwrap();

    let ws_stream = tokio_tungstenite::accept_hdr_async(stream, copy_headers_callback)
        .await
        .expect("Error during the websocket handshake occurred");

    info!("New WebSocket connection: {}", addr);

    let (mut write, read) = ws_stream.split();

    //fake_boot(write).await;

    let mut boot_status = goval::Command::default();
    boot_status.body = Some(goval::command::Body::BootStatus(
        goval::BootStatus::default(),
    ));

    send_message(boot_status, &mut write)
        .await
        .expect("Error sending spoofed boot status");

    // Sending container state
    let mut container_state = goval::Command::default();
    let mut inner_state = goval::ContainerState::default();
    inner_state.state = goval::container_state::State::Ready.into();
    container_state.body = Some(goval::command::Body::ContainerState(inner_state));

    send_message(container_state, &mut write)
        .await
        .expect("Error sending spoofed container state");

    // Sending server info message
    let mut toast = goval::Command::default();
    let mut inner_state = goval::Toast::default();
    inner_state.text = "Hello from an unnamed goval impl".to_owned();
    toast.body = Some(goval::command::Body::Toast(inner_state));

    send_message(toast, &mut write)
        .await
        .expect("Error sending server info toast");

    SESSION_CHANNELS.write(session).insert(vec![]);
    tokio::spawn(async move {
        if let Err(err) = read
            .try_for_each(|msg| {
                match msg {
                    tungstenite::Message::Binary(buf) => {
                        // let decode = buf.clone();
                        if let Err(err) = propogate
                            .send(IPCMessage {
                                bytes: buf,
                                session,
                            }) {
                                error!("The following error occured when enqueing message to global messagq queue (session: {}): {}", session, err)
                            }
                    }
                    tungstenite::Message::Close(_) => {
                        warn!("CLOSING SESSION {}", session);
                        for channel in SESSION_CHANNELS.read(&session).get().unwrap().iter() {
                            CHANNEL_MESSAGES
                                .read(&channel)
                                .get()
                                .unwrap()
                                .push(JsMessage::Close(session));
                        }
                        warn!("CLOSED SESSION {}", session);
                    }
                    _ => {}
                };
                future::ok(())
            })
            .await
        {
            error!(
                "The following error occured while reading messages (session {}): {}",
                session, err
            );
        }
    });

    while let Some(i) = sent.recv().await {
        match write.send(tungstenite::Message::Binary(i.bytes)).await {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "The following error occured while sending a message: {}",
                    err
                )
            }
        }
    }
}

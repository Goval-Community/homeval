use deno_core::error::AnyError;
#[allow(unused_imports)]
use homeval::goval;
use homeval::goval::Command;
use prost::Message;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use std::{env, io::Error, sync::Arc};

use futures_util::{future, SinkExt, StreamExt, TryStreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex, RwLock},
};
use log::{debug, error, info, warn};

mod channels;
use channels::IPCMessage;

mod deno_extension;
use deno_extension::{make_extension, JsMessage, Service};

mod parse_paseto;
use parse_paseto::{parse, ClientInfo};

#[cfg(debug_assertions)]
mod services_debug;
#[cfg(debug_assertions)]
use services_debug as services;

#[cfg(not(debug_assertions))]
mod services_release;
#[cfg(not(debug_assertions))]
use services_release as services;

use lazy_static::lazy_static;
lazy_static! {
    pub static ref IMPLEMENTED_SERVICES: Vec<String> = {
        let mut services = vec![];
        for file in services::get_all().expect("Error when looping over `services/`") {
            if let Some(extension) = file.extension() {
                if extension == "js" {
                    let mut service_path = file
                        .file_name()
                        .expect("File name missing while extension exists???")
                        .to_str()
                        .expect("failed to convert path OsString to String")
                        .to_string();

                    service_path
                        .pop()
                        .expect("Impossible case when removing .js extension from service file");
                    service_path
                        .pop()
                        .expect("Impossible case when removing .js extension from service file");
                    service_path
                        .pop()
                        .expect("Impossible case when removing .js extension from service file");

                    services.push(service_path);
                }
            }
        }
        services
    };
}

lazy_static! {
    static ref MAX_SESSION: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
    static ref MAX_CHANNEL: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

use c_map::HashMap;
lazy_static! {
    static ref SESSION_CHANNELS: HashMap<i32, Vec<i32>> = HashMap::new();
    static ref SESSION_CLIENT_INFO: HashMap<i32, ClientInfo> = HashMap::new();
    static ref CHANNEL_MESSAGES: Arc<RwLock<std::collections::HashMap<i32, Arc<deadqueue::unlimited::Queue<JsMessage>>>>> =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    static ref CHANNEL_METADATA: RwLock<Vec<Service>> = RwLock::new(vec![]);
    static ref SESSION_MAP: Arc<HashMap<i32, mpsc::UnboundedSender<IPCMessage>>> =
        Arc::new(HashMap::new());
    static ref PTY_WRITE_MESSAGES: HashMap<u32, Arc<deadqueue::unlimited::Queue<String>>> =
        HashMap::new();
    static ref PTY_CHANNEL_TO_ID: HashMap<i32, Vec<u32>> = HashMap::new();
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    console_subscriber::init();

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
            error!(
                "Following error occured when decoding message in main loop: {}",
                err
            );
            continue;
        } else {
            cmd = command_res
                .expect("Impossible condition, decoded message was already verified as not None")
        }

        let cmd_body: goval::command::Body;

        match cmd.body {
            None => {
                error!("MISSING COMMAND BODY: {:#?}", cmd);
                continue;
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
                            info!("executing openchan main block");
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

                            info!("Awaiting queue write for channel {}", channel_id);

                            CHANNEL_MESSAGES.write().await.insert(
                                channel_id_held,
                                Arc::new(deadqueue::unlimited::Queue::new()),
                            );
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
                                            let mod_path =
                                                &format!("services/{}.js", open_chan.service);
                                            debug!("Module path: {}", mod_path);
                                            let main_module: deno_core::url::Url;
                                            let main_module_res = deno_core::resolve_path(mod_path);
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
                                                    include_str!(concat!(env!("OUT_DIR"), "/api.js")),
                                                )
                                                .unwrap();

                                            let mod_id = js_runtime
                                                .load_main_module(
                                                    &main_module,
                                                    services::get_module_core(open_chan.service)
                                                        .expect("Error fetching module code")
                                                )
                                                .await
                                                .unwrap();
                                            let result = js_runtime.mod_evaluate(mod_id);

                                            js_runtime.run_event_loop(false).await.unwrap();
                                            result.await.unwrap().unwrap();
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

                        let msg_read = CHANNEL_MESSAGES.read().await;

                        let queue = msg_read.get(&channel_id_held).unwrap().clone();

                        drop(msg_read);

                        queue.push(JsMessage::Attach(message.session));

                        SESSION_CHANNELS
                            .write(message.session)
                            .entry()
                            .and_modify(|channels| channels.push(channel_id_held));
                    }
                }
                _ => {}
            }
        } else {
            // Directly deal with Command::Input, should be faster
            if let goval::command::Body::Input(input) = cmd_body {
                if let Some(ptys) = PTY_CHANNEL_TO_ID.read(&cmd.channel).get() {
                    if ptys.len() == 1 {
                        let pty_id = ptys[0];
                        let mut to_continue = false;
                        if let Some(queue) = crate::PTY_WRITE_MESSAGES.read(&pty_id).get() {
                            queue.push(input);
                            to_continue = true;
                        } else {
                            error!("Couldn't find pty {} to write to", pty_id);
                        }

                        if to_continue {
                            continue;
                        }
                    }
                }
            }

            let msg_lock = CHANNEL_MESSAGES.read().await;

            let queue = msg_lock.get(&cmd.channel).unwrap().clone();

            drop(msg_lock);

            queue.push(JsMessage::IPC(message));
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
    propagate: mpsc::UnboundedSender<IPCMessage>,
    mut sent: mpsc::UnboundedReceiver<IPCMessage>,
    session: i32,
) {
    let addr = stream
        .peer_addr()
        .expect("connected streams should have a peer address");
    info!("Peer address: {}", addr);

    let _uri: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));

    let copy_headers_callback = |req: &Request, res: Response| -> Result<Response, ErrorResponse> {
        let mut ptr = _uri.lock().unwrap();
        *ptr = Some(req.uri().path().to_string());
        drop(ptr);

        Ok(res)
    };

    let ws_stream = tokio_tungstenite::accept_hdr_async(stream, copy_headers_callback)
        .await
        .expect("Error during the websocket handshake occurred");

    // needed due to futures stuff :/
    let client: ClientInfo;
    match tokio::task::spawn_blocking(move || -> Result<ClientInfo, AnyError> {
        let __uri = _uri
            .lock()
            .expect("Header callback paniced when setting uri");

        let uri = __uri.clone().unwrap_or("".to_string());

        let split = uri.split("/").collect::<Vec<&str>>();
        if split.len() >= 3 {
            return Ok(parse(split[2])?);
        } else {
            warn!("Request path wasnt long enough, using default user")
        }

        Ok(ClientInfo::default())
    })
    .await
    .expect("Error converting uri to client info")
    {
        Ok(_client) => client = _client,
        Err(err) => {
            warn!(
                "Error while decoding request path: {}, using default user",
                err
            );
            client = ClientInfo::default();
        }
    }

    info!("New connection from: {:#?}", client);

    SESSION_CLIENT_INFO.write(session).insert(client);

    info!("New WebSocket connection: {}", addr);

    let (mut write, read) = ws_stream.split();

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
                        if let Err(err) = propagate
                            .send(IPCMessage {
                                bytes: buf,
                                session,
                            }) {
                                error!("The following error occured when enqueing message to global messagq queue (session: {}): {}", session, err)
                            }
                    }
                    tungstenite::Message::Close(_) => {
                        warn!("CLOSING SESSION {}", session);
                        for _channel in SESSION_CHANNELS.read(&session).get().unwrap().iter() {
                            let channel = _channel.clone();
                            tokio::spawn(async move {
                                let msg_lock = CHANNEL_MESSAGES
                                .read().await;

                                let queue = msg_lock.get(&channel)
                                    .unwrap()
                                    .clone();
                                
                                drop(msg_lock);
                                
                                queue.push(JsMessage::Close(session));
                            });
                            
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

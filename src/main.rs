#[cfg(feature = "fun-stuff")]
use chrono::Datelike;
use deno_core::error::AnyError;
use deno_extension::messaging::ReplspaceMessage;
#[allow(unused_imports)]
use homeval::goval;
use homeval::goval::Command;
use prost::Message;
use std::time::Instant;
use std::{collections::HashMap, env, io::Error, sync::Arc};
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

use futures_util::{SinkExt, StreamExt};
use log::{error, info, trace, warn};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex, RwLock},
};

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

mod config;
use config::dotreplit::DotReplit;

#[cfg(feature = "replspace")]
mod replspace;

use lazy_static::lazy_static;
lazy_static! {
    pub static ref START_TIME: Instant = Instant::now();
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
    pub static ref DOTREPLIT_CONFIG: DotReplit =
        toml::from_str(&std::fs::read_to_string(".replit").unwrap_or("".to_string())).unwrap();
}

lazy_static! {
    static ref MAX_SESSION: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
    static ref MAX_CHANNEL: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

lazy_static! {
    static ref SESSION_CHANNELS: RwLock<HashMap<i32, Vec<i32>>> = RwLock::new(HashMap::new());
    static ref SESSION_CLIENT_INFO: RwLock<HashMap<i32, ClientInfo>> = RwLock::new(HashMap::new());
    static ref CHANNEL_MESSAGES: Arc<RwLock<HashMap<i32, Arc<deadqueue::unlimited::Queue<JsMessage>>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    static ref CHANNEL_METADATA: RwLock<Vec<Service>> = RwLock::new(vec![]);
    static ref SESSION_MAP: Arc<RwLock<HashMap<i32, mpsc::UnboundedSender<IPCMessage>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // pty and cmd's
    static ref PROCCESS_WRITE_MESSAGES: RwLock<HashMap<u32, Arc<deadqueue::unlimited::Queue<String>>>> =
    RwLock::new(HashMap::new());
    static ref PROCCESS_CHANNEL_TO_ID: RwLock<HashMap<i32, u32>> = RwLock::new(HashMap::new());

    static ref CPU_STATS: Arc<cpu_time::ProcessTime> = Arc::new(cpu_time::ProcessTime::now());

    // Hashmap for channel id -> last session that sent it a message
    static ref LAST_SESSION_USING_CHANNEL: Arc<RwLock<HashMap<i32, i32>>> = Arc::new(RwLock::new(HashMap::new()));

    static ref REPLSPACE_CALLBACKS: Arc<RwLock<HashMap<String, Option<tokio::sync::oneshot::Sender<ReplspaceMessage>>>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

#[cfg(feature = "database")]
mod database;
#[cfg(feature = "database")]
pub use database::DATABASE;

#[tokio::main]
async fn main() -> Result<(), Error> {
    lazy_static::initialize(&START_TIME);
    // console_subscriber::init();
    let _ = env_logger::try_init().unwrap();
    
    #[cfg(feature = "database")]
    database::setup().await.unwrap();

    info!("Starting homeval!");
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    #[cfg(feature = "replspace")]
    tokio::spawn(replspace::start_server());

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
            session_map_clone
                .write()
                .await
                .insert(session_id, send_to_session);

            let tx_clone = tx.clone();
            tokio::spawn(async move {
                match accept_connection(stream, tx_clone, session_recv, session_id).await {
                    Ok(_) => {}
                    Err(err) => {
                        error!("accept_connection errored: {}", err)
                    }
                }
            });
        }
    });

    let session_map_clone = SESSION_MAP.clone();
    while let Some(message) = rx.recv().await {
        let cmd: Command;
        match message.to_cmd() {
            Err(err) => {
                error!(
                    "Following error occured when decoding message in main loop: {}",
                    err
                );
                continue;
            }
            Ok(res) => cmd = res,
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

                    if let Some(sender) = session_map_clone.read().await.get(&message.session) {
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

                            session_map_clone
                                .read()
                                .await
                                .get(&message.session)
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
                        continue;
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
) -> Result<(), AnyError> {
    let addr = stream.peer_addr()?;
    trace!("New connection with peer address: {}", addr);

    let _uri: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));

    let copy_headers_callback = |req: &Request, res: Response| -> Result<Response, ErrorResponse> {
        let mut ptr = _uri.lock().unwrap();
        *ptr = Some(req.uri().path().to_string());
        drop(ptr);

        Ok(res)
    };

    let ws_stream = tokio_tungstenite::accept_hdr_async(stream, copy_headers_callback).await?;

    // needed due to futures stuff :/
    let client: ClientInfo;
    match tokio::task::spawn_blocking(move || -> Result<ClientInfo, AnyError> {
        let __uri;
        match _uri.lock() {
            Ok(res) => __uri = res,
            Err(_err) => {
                return Err(Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Header callback paniced when setting uri",
                )
                .into())
            }
        }

        let uri = __uri.clone().unwrap_or("".to_string());

        let split = uri.split("/").collect::<Vec<&str>>();
        if split.len() >= 3 {
            return Ok(parse(split[2])?);
        } else {
            warn!("Request path wasnt long enough, using default user")
        }

        Ok(ClientInfo::default())
    })
    .await?
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

    info!("New client: {:#?}", client);

    SESSION_CLIENT_INFO
        .write()
        .await
        .insert(session, client.clone());

    trace!("New WebSocket connection: {}", addr);

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
                    tungstenite::Message::Binary(buf) => {
                        if let Err(err) = propagate.send(IPCMessage {
                            bytes: buf,
                            session,
                        }) {
                            error!("The following error occured when enqueing message to global messagq queue (session: {}): {}", session, err)
                        }
                    }
                    tungstenite::Message::Close(_) => {
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

    Ok(())
}

#[allow(unused_imports)]
use goval_impl::goval;
use prost::Message;
use tokio_tungstenite::tungstenite;
//use std::fmt::Binary;
use std::{env, io::Error, sync::Arc};

use futures_util::{future, SinkExt, StreamExt, TryStreamExt};
use tokio;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex, RwLock},
};
//use async_log::span;
use log::info;
mod channels;
use channels::IPCMessage;

mod deno_extension;
use deno_extension::{make_extension, Service};

use lazy_static::lazy_static;

lazy_static! {
    static ref MAX_SESSION: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
    static ref MAX_CHANNEL: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

use c_map::HashMap;
lazy_static! {
    static ref CHANNEL_MESSAGES: HashMap<i32, deadqueue::unlimited::Queue<IPCMessage>> =
        HashMap::new();
    static ref SESSION_MAP: Arc<HashMap<i32, mpsc::UnboundedSender<IPCMessage>>> =
        Arc::new(HashMap::new());
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
    while let Some(i) = rx.recv().await {
        let cmd = i.to_cmd().unwrap();
        //println!("{}: {:#?}", i.session, cmd);

        if cmd.channel == 0 {
            match cmd.body.unwrap() {
                goval::command::Body::Ping(_) => {
                    let mut pong = goval::Command::default();
                    pong.body = Some(goval::command::Body::Pong(goval::Pong::default()));
                    pong.r#ref = cmd.r#ref;
                    pong.channel = 0;

                    session_map_clone
                        .read(&i.session)
                        .get()
                        .unwrap()
                        .send(IPCMessage::from_cmd(pong, i.session))
                        .unwrap();
                }
                goval::command::Body::OpenChan(open_chan) => {
                    if open_chan.service == "gcsfiles".to_owned() {
                        info!("executing openchan main block");
                        let service = open_chan.service.clone();
                        let mut max_channel = MAX_CHANNEL.lock().await;
                        *max_channel += 1;
                        let channel_id = max_channel.clone();
                        let channel_id_held = channel_id.clone();
                        drop(max_channel);

                        CHANNEL_MESSAGES
                            .write(channel_id_held)
                            .insert(deadqueue::unlimited::Queue::new());
                        info!("Added channel: {} to queue list", channel_id_held);

                        tokio::task::spawn_blocking(move || {
                            let local = tokio::task::LocalSet::new();

                            let rt = tokio::runtime::Handle::current();
                            rt.block_on(async {
                                local
                                    .run_until(async {
                                        // local.spawn_local(async move {
                                        let main_module =
                                            deno_core::resolve_path("service.js").unwrap();
                                        let mut js_runtime =
                                            deno_core::JsRuntime::new(deno_core::RuntimeOptions {
                                                module_loader: Some(std::rc::Rc::new(
                                                    deno_core::FsModuleLoader,
                                                )),
                                                extensions: vec![make_extension()],
                                                ..Default::default()
                                            });

                                        js_runtime
                                            .execute_script(
                                                "[goval::generated::globals]",
                                                &format!(
                                                    "globalThis.serviceInfo = {};",
                                                    serde_json::to_string(&Service {
                                                        service: service,
                                                        id: channel_id,
                                                        name: None
                                                    })
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

                        let mut open_chan_res = goval::Command::default();
                        let mut _open_res = goval::OpenChannelRes::default();
                        _open_res.state = goval::open_channel_res::State::Created.into();
                        _open_res.id = channel_id_held;
                        open_chan_res.body = Some(goval::command::Body::OpenChanRes(_open_res));
                        open_chan_res.r#ref = cmd.r#ref;
                        open_chan_res.channel = 0;

                        session_map_clone
                            .read(&i.session)
                            .get()
                            .unwrap()
                            .send(IPCMessage::from_cmd(open_chan_res, i.session))
                            .unwrap();
                    }
                }
                _ => {}
            }
        } else {
            CHANNEL_MESSAGES.read(&cmd.channel).get().unwrap().push(i);
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
) -> Result<(), tungstenite::Error> {
    let mut buf = Vec::new();
    buf.reserve(message.encoded_len());
    message.encode(&mut buf).unwrap();

    stream.send(tungstenite::Message::Binary(buf)).await
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

    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("Error during the websocket handshake occurred");

    info!("New WebSocket connection: {}", addr);

    let (mut write, read) = ws_stream.split();

    //fake_boot(write).await;

    let mut boot_status = goval::Command::default();
    boot_status.body = Some(goval::command::Body::BootStatus(
        goval::BootStatus::default(),
    ));

    send_message(boot_status, &mut write).await.unwrap();

    // Sending container state
    let mut container_state = goval::Command::default();
    let mut inner_state = goval::ContainerState::default();
    inner_state.state = goval::container_state::State::Ready.into();
    container_state.body = Some(goval::command::Body::ContainerState(inner_state));

    send_message(container_state, &mut write).await.unwrap();

    // Sending server info message
    let mut toast = goval::Command::default();
    let mut inner_state = goval::Toast::default();
    inner_state.text = "Hello from an unnamed goval impl".to_owned();
    toast.body = Some(goval::command::Body::Toast(inner_state));

    send_message(toast, &mut write).await.unwrap();

    tokio::spawn(async move {
        read.try_for_each(|msg| {
            match msg {
                tungstenite::Message::Binary(buf) => {
                    // let decode = buf.clone();
                    propogate
                        .send(IPCMessage {
                            bytes: buf,
                            session,
                        })
                        .unwrap();
                }
                _ => {}
            };
            future::ok(())
        })
        .await
        .unwrap();
    });

    while let Some(i) = sent.recv().await {
        write
            .send(tungstenite::Message::Binary(i.bytes))
            .await
            .unwrap();
    }
}

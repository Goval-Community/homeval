use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        ConnectInfo, Path, State,
    },
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

#[cfg(feature = "fun-stuff")]
use chrono::Datelike;

use anyhow::Result;
use goval::{Command, OpenChannel};
use homeval_services::{ClientInfo, ServiceMetadata};
use prost::Message;
use std::{net::SocketAddr, sync::LazyLock};
use tokio::sync::{mpsc::UnboundedSender, Mutex};

use futures_util::{SinkExt, StreamExt};
use log::{as_debug, as_display, as_error, debug, error, info, trace, warn};
use tokio::sync::mpsc;

use crate::{
    CHANNEL_MESSAGES, CHANNEL_METADATA, CHANNEL_SESSIONS, LAST_SESSION_USING_CHANNEL, MAX_SESSION,
    PROCCESS_CHANNEL_TO_ID, SESSION_CHANNELS, SESSION_CLIENT_INFO, SESSION_MAP,
};

use crate::{parse_paseto::parse, ChannelMessage, IPCMessage};

#[derive(Clone)]
struct AppState {
    sender: UnboundedSender<IPCMessage>,
}

static DEFAULT_REPLY: &str = "(づ ◕‿◕ )づ Hello there";

pub async fn start_server() -> Result<()> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let (tx, mut rx) = mpsc::unbounded_channel::<IPCMessage>();

    let app = Router::new()
        .route("/wsv2/:token", get(wsv2))
        .fallback(get(default_handler))
        .with_state(AppState { sender: tx });
    info!("Goval server listening on: {}", addr);

    tokio::spawn(async move {
        axum::Server::bind(&addr.parse().unwrap())
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap()
    });

    let max_channel = Mutex::new(0);

    while let Some(message) = rx.recv().await {
        handle_message(message, &SESSION_MAP, &max_channel).await;
    }

    Ok(())
}

async fn default_handler() -> Response {
    DEFAULT_REPLY.into_response()
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
        parse(&token).await.unwrap_or(ClientInfo::default()),
        addr,
    )
    .await
    {
        Ok(_) => {}
        Err(err) => {
            error!(error = as_display!(err); "accept_connection errored")
        }
    };
}

async fn wsv2(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| on_wsv2_upgrade(socket, token, state, addr))
}

async fn handle_message(
    message: IPCMessage,
    session_map: &LazyLock<
        tokio::sync::RwLock<std::collections::HashMap<i32, mpsc::UnboundedSender<IPCMessage>>>,
    >,
    max_channel: &Mutex<i32>,
) {
    let cmd: Command = message.clone().command;

    let cmd_body: goval::command::Body;

    match cmd.body {
        None => {
            error!(command = as_debug!(cmd); "MISSING COMMAND BODY");
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
                            error!(error = as_error!(err);"Error occured while sending Pong");
                        }
                    }
                } else {
                    error!("Missing session queue when sending Pong")
                }
            }
            goval::command::Body::OpenChan(open_chan) => {
                if let Err(err) = open_channel(open_chan, message, max_channel, session_map).await {
                    error!(error = as_display!(err); "Error in open chan handler")
                }
            }

            goval::command::Body::CloseChan(close_chan) => {
                // TODO: follow close_chan.action
                tokio::spawn(async move {
                    match detach_channel(close_chan.id, message.session, true).await {
                        Ok(_) => {}
                        Err(err) => {
                            error!(error = as_display!(err), session = message.session, channel = close_chan.id;
                            "Error occured while detaching from channel")
                        }
                    }
                });
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
                    error!(pty_id = pty_id; "Couldn't find pty to write to");
                }

                if to_continue {
                    return;
                }
            }
        }

        let msg_lock = CHANNEL_MESSAGES.read().await;

        let queue = msg_lock.get(&cmd.channel).unwrap().clone();

        drop(msg_lock);

        queue
            .send(ChannelMessage::IPC(message.clone()))
            .expect("TODO: deal with this");

        let mut hashmap_lock = LAST_SESSION_USING_CHANNEL.write().await;

        hashmap_lock.insert(cmd.channel, message.session);

        drop(hashmap_lock);
    }
}

async fn open_channel(
    open_chan: OpenChannel,
    message: IPCMessage,
    max_channel: &Mutex<i32>,
    session_map: &LazyLock<
        tokio::sync::RwLock<std::collections::HashMap<i32, mpsc::UnboundedSender<IPCMessage>>>,
    >,
) -> Result<()> {
    let searcher: &str = &open_chan.service;
    if homeval_services::IMPLEMENTED_SERVICES.contains(&searcher) {
        let mut found = false;
        let mut channel_id_held = 0;

        let attach = open_chan.action() == goval::open_channel::Action::AttachOrCreate
            || open_chan.action() == goval::open_channel::Action::Attach
            || open_chan.service == "git"; // git is just use for replspace api stuff
                                           // from what I can tell, so its just easier to have it as one instance.
        let create = open_chan.action() == goval::open_channel::Action::AttachOrCreate
            || open_chan.action() == goval::open_channel::Action::Create;
        if attach {
            let metadata = CHANNEL_METADATA.read().await;
            for (id, channel) in metadata.iter() {
                if channel.name.is_some()
                    && channel.name.clone().unwrap_or("".to_string()) == open_chan.name
                    && channel.service.clone() == open_chan.service
                {
                    found = true;
                    channel_id_held = id.clone();
                    continue;
                }
            }
        }

        if !found && create {
            trace!("executing openchan main block");
            let service = open_chan.service.clone();
            let mut max_channel = max_channel.lock().await;
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

            let service_data = ServiceMetadata {
                service: service.clone(),
                id: channel_id,
                name: _channel_name.clone(),
            };

            trace!(channel = channel_id; "Awaiting queue write");

            let (writer, reader) = mpsc::unbounded_channel();

            CHANNEL_MESSAGES
                .write()
                .await
                .insert(channel_id_held, writer.clone());

            let mut metadata = CHANNEL_METADATA.write().await;
            metadata.insert(channel_id_held, service_data.clone());
            drop(metadata);
            trace!(channel = channel_id_held; "Added channel to queue list");

            tokio::spawn(async move {
                let channel =
                    homeval_services::Channel::new(channel_id, service, _channel_name, writer)
                        .await
                        .expect("TODO: Deal with this");
                channel.start(reader).await;
            });
            found = true;
        }

        if !found {
            error!("Couldnt make channel");
            let mut protocol_error = goval::Command::default();
            let mut _inner = goval::ProtocolError::default();
            _inner.text = "Could not create / attach channel".to_string();

            protocol_error.body = Some(goval::command::Body::ProtocolError(_inner));
            protocol_error.r#ref = message.command.r#ref.clone();
            protocol_error.channel = 0;

            session_map
                .read()
                .await
                .get(&message.session)
                .unwrap()
                .send(message.replace_cmd(protocol_error))
                .unwrap();
            return Ok(());
        }

        let mut open_chan_res = goval::Command::default();
        let mut _open_res = goval::OpenChannelRes::default();
        _open_res.state = goval::open_channel_res::State::Created.into();
        _open_res.id = channel_id_held;
        open_chan_res.body = Some(goval::command::Body::OpenChanRes(_open_res));
        open_chan_res.r#ref = message.command.r#ref.clone();
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

        queue.send(ChannelMessage::Attach(
            message.session,
            SESSION_CLIENT_INFO
                .read()
                .await
                .get(&message.session)
                .unwrap()
                .clone(),
            SESSION_MAP
                .read()
                .await
                .get(&message.session)
                .expect("TODO: deal with this")
                .clone(),
        ))?;

        let mut guard = CHANNEL_SESSIONS.write().await;

        match guard.get_mut(&channel_id_held) {
            Some(arr) => {
                arr.push(message.session);
            }
            None => {
                guard.insert(channel_id_held, vec![message.session]);
            }
        }

        drop(guard);

        SESSION_CHANNELS
            .write()
            .await
            .entry(message.session)
            .and_modify(|channels| channels.push(channel_id_held));
    } else {
        warn!(
            service = open_chan.service;
            "Missing service requested by openChan"
        )
    }
    Ok(())
}

async fn detach_channel(channel: i32, session: i32, forced: bool) -> Result<()> {
    trace!(session = session, channel = channel, forced = forced; "Client is closing a channel");

    SESSION_CHANNELS
        .write()
        .await
        .entry(session)
        .and_modify(|channels| channels.retain(|chan: &i32| chan.clone() != channel));

    let msg_lock = CHANNEL_MESSAGES.read().await;

    let queue = msg_lock.get(&channel).unwrap().clone();

    drop(msg_lock);

    queue.send(ChannelMessage::Detach(session))?;

    trace!("Waiting for sessions lock");
    let mut guard = CHANNEL_SESSIONS.write().await;
    trace!("Done waiting for sessions lock");

    match guard.get_mut(&channel) {
        Some(arr) => {
            arr.retain(|sess| sess.clone() != session);
            if arr.len() == 0 {
                trace!(channel = channel; "Shutting down channel");
                queue.send(ChannelMessage::Shutdown)?;

                tokio::spawn(async move {
                    CHANNEL_METADATA.write().await.remove(&channel);
                    CHANNEL_SESSIONS.write().await.remove(&channel);
                    PROCCESS_CHANNEL_TO_ID.write().await.remove(&channel);
                    LAST_SESSION_USING_CHANNEL.write().await.remove(&channel);
                });
            } else {
                trace!(sessions = as_debug!(arr); "Sessions still remain on channel");
            }
        }
        None => {
            warn!(channel = channel; "Missing CHANNEL_SESSIONS");
        }
    }

    drop(guard);
    Ok(())
}

async fn send_message(
    message: goval::Command,
    stream: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
) -> Result<()> {
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
    addr: SocketAddr,
) -> Result<()> {
    info!(peer_address = as_display!(addr); "New connection");

    info!(client = as_debug!(client); "New client");

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
                        let _message: anyhow::Result<IPCMessage> = buf.try_into();
                        let message: IPCMessage;
                        match _message {
                            Ok(mut msg) => {
                                msg.session = session;
                                message = msg
                            }
                            Err(err) => {
                                error!(error = as_display!(err), session = session; "Error decoding message from client");
                                continue;
                            }
                        }

                        if let Err(err) = propagate.send(message) {
                            error!(session = session, error = as_error!(err); "An error occured when enqueing message to global message queue")
                        }
                    }
                    WsMessage::Close(_) => {
                        warn!(session = session; "CLOSING SESSION");
                        for _channel in SESSION_CHANNELS.read().await.get(&session).unwrap().iter()
                        {
                            let channel = _channel.clone();
                            tokio::spawn(async move {
                                match detach_channel(channel, session, true).await {
                                    Ok(_) => {}
                                    Err(err) => {
                                        error!(error = as_display!(err), session = session, channel = channel; "Error occured while detaching from channel")
                                    }
                                }
                            });
                        }

                        SESSION_MAP.write().await.remove(&session);
                        SESSION_CLIENT_INFO.write().await.remove(&session);
                        SESSION_CHANNELS.write().await.remove(&session);
                        warn!(session = session;  "CLOSED SESSION");
                    }
                    _ => {}
                },
                Err(err) => {
                    error!(
                        session = session, error = as_error!(err);
                        "An error occured while reading messages"
                    );
                }
            };
        }
    });

    while let Some(i) = sent.recv().await {
        match write.send(WsMessage::Binary(i.to_bytes())).await {
            Ok(_) => {}
            Err(err) => {
                error!(
                    session = i.session, error = as_error!(err);
                    "An error occured while sending a message"
                )
            }
        }
    }

    Ok(())
}

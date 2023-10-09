mod chat;
mod dotreplit;
mod gcsfiles;
mod git;
mod ot;
mod output;
mod presence;
mod shell;
mod snapshot;
mod stub;
mod toolchain;
mod traits;
mod types;

use anyhow::format_err;
use anyhow::Result;
use log::as_display;
use log::error;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use types::config::dotreplit::DotReplit;
pub use types::*;

pub struct Channel {
    info: ChannelInfo,
    _inner: Box<dyn traits::Service + Send>,
}

// Public functions
impl Channel {
    pub async fn new(
        id: i32,
        service: String,
        name: Option<String>,
        dotreplit: Arc<RwLock<DotReplit>>,
        sender: tokio::sync::mpsc::UnboundedSender<ChannelMessage>,
    ) -> Result<Channel> {
        let info = ChannelInfo {
            id,
            name,
            service: service.clone(),
            clients: HashMap::new(),
            sessions: HashMap::new(),
            sender: sender.clone(),
            dotreplit,
        };

        let channel: Box<dyn traits::Service + Send> = match service.as_str() {
            "chat" => Box::new(chat::Chat::new()),
            "gcsfiles" => Box::new(gcsfiles::GCSFiles {}),
            "presence" => Box::new(presence::Presence::new()),
            "ot" => Box::new(ot::OT::new(sender).await?),
            "snapshot" => Box::new(snapshot::Snapshot {}),
            "output" => Box::new(output::Output::new().await),
            "shell" => Box::new(shell::Shell::new(&info).await?),
            "toolchain" => Box::new(toolchain::Toolchain {}),
            "git" => Box::new(git::Git::new()),
            "dotreplit" => Box::new(dotreplit::DotReplit {}),
            "null" => Box::new(stub::Stub {}), // This channel never does anything
            "open" => Box::new(stub::Stub {}), // Stub until infra is set up to handle this
            _ => return Err(format_err!("Unknown service: {}", service)),
        };

        Ok(Channel {
            info,
            _inner: channel,
        })
    }

    pub async fn start(mut self, mut read: tokio::sync::mpsc::UnboundedReceiver<ChannelMessage>) {
        while let Some(message) = read.recv().await {
            let result = match message {
                ChannelMessage::Attach(session, client, sender) => {
                    self.attach(session, client, sender).await
                }
                ChannelMessage::Detach(session) => self.detach(session).await,
                ChannelMessage::IPC(ipc) => self.message(ipc.command, ipc.session).await,
                ChannelMessage::ProcessDead(exit_code) => {
                    self._inner.proccess_died(&self.info, exit_code).await
                }
                ChannelMessage::CmdDead(_) => todo!(),
                ChannelMessage::Replspace(session, msg, respond) => {
                    self._inner
                        .replspace(&self.info, msg, session, respond)
                        .await
                }
                ChannelMessage::Shutdown => match self._inner.shutdown(&self.info).await {
                    Ok(_) => break,
                    Err(err) => {
                        error!(error = as_display!(err); "Error encountered in Service#shutdown");
                        break;
                    }
                },
                ChannelMessage::FSEvent(event) => self._inner.fsevent(&self.info, event).await,
            };

            match result {
                Ok(_) => {}
                Err(err) => {
                    error!(error = as_display!(err); "Error encountered in service")
                }
            }
        }
    }
}

// Private functions
impl Channel {
    async fn message(&mut self, message: goval::Command, session: i32) -> Result<()> {
        match self
            ._inner
            .message(&self.info, message.clone(), session)
            .await?
        {
            Some(mut msg) => {
                msg.r#ref = message.r#ref;
                self.info.send(msg, SendSessions::Only(session)).await?
            }
            None => {}
        }
        Ok(())
    }

    async fn attach(
        &mut self,
        session: i32,
        client: ClientInfo,
        sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<()> {
        self.info.sessions.insert(session, client.clone());
        self.info.clients.insert(session, sender.clone());
        match self
            ._inner
            .attach(&self.info, client, session, sender)
            .await?
        {
            None => {}
            Some(msg) => {
                self.info.send(msg, SendSessions::Only(session)).await?;
            }
        }
        Ok(())
    }

    async fn detach(&mut self, session: i32) -> Result<()> {
        self.info.sessions.retain(|sess, _| sess != &session);
        self.info.clients.retain(|sess, _| sess != &session);
        self._inner.detach(&self.info, session).await?;
        Ok(())
    }
}

pub static IMPLEMENTED_SERVICES: &[&str] = &[
    "chat",
    "gcsfiles",
    "presence",
    "ot",
    "snapshot",
    "null",
    "git",
    "open",
    "output",
    "shell",
    "toolchain",
    "dotreplit",
];

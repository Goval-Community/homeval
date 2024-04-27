#![feature(extract_if)]
#![warn(
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::redundant_clone,
    clippy::missing_const_for_fn
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,

    // TODO: investigate performance impact of i__::try_from
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,

    // TODO: remove and fix these 
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

mod chat;
mod dotreplit;
mod exec;
mod fsevents;
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
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::error;
pub use types::*;

pub struct Channel {
    info: ChannelInfo,
    inner: Box<dyn traits::Service + Send>,
}

// Public functions
impl Channel {
    pub async fn new(
        id: ChannelID,
        service: String,
        name: Option<String>,
        dotreplit: Arc<RwLock<DotReplit>>,
        child_env_vars: Arc<RwLock<HashMap<String, String>>>,
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
            child_env_vars,
        };

        let channel: Box<dyn traits::Service + Send> = match service.as_str() {
            "chat" => Box::new(chat::Chat::new()),
            "gcsfiles" => Box::new(gcsfiles::GCSFiles {}),
            "presence" => Box::new(presence::Presence::new()),
            "ot" => Box::new(ot::OT::new(sender).await?),
            "snapshot" => Box::new(snapshot::Snapshot {}),
            "output" => Box::new(output::Output::new()),
            "shell" => Box::new(shell::Shell::new(&info)?),
            "toolchain" => Box::new(toolchain::Toolchain {}),
            "git" => Box::new(git::Git::new()),
            "exec" => Box::new(exec::Exec::new()),
            "dotreplit" => Box::new(dotreplit::DotReplit {}),
            "fsevents" => Box::new(fsevents::FSEvents::new(sender).await?),
            "audio" | "null" => Box::new(stub::Stub {}), // Audio will never be supported and null does nothing
            _ => return Err(format_err!("Unknown service: {}", service)),
        };

        Ok(Channel {
            info,
            inner: channel,
        })
    }

    pub async fn start(mut self, mut read: tokio::sync::mpsc::UnboundedReceiver<ChannelMessage>) {
        if let Err(err) = self.inner.open(&self.info).await {
            error!(%err, "Error encountered in Service::open");
        }

        while let Some(message) = read.recv().await {
            let result = match message {
                ChannelMessage::Attach(session, client, sender) => {
                    self.attach(session, client, sender).await
                }
                ChannelMessage::Detach(session) => self.detach(session).await,
                ChannelMessage::IPC(ipc) => self.message(ipc.command, ipc.session).await,
                ChannelMessage::ProcessDead(exit_code) => {
                    self.inner.proccess_died(&self.info, exit_code).await
                }
                ChannelMessage::Replspace(session, msg, respond) => {
                    self.inner
                        .replspace(&self.info, msg, session, respond)
                        .await
                }
                ChannelMessage::Shutdown => match self.inner.shutdown(&self.info).await {
                    Ok(()) => break,
                    Err(err) => {
                        error!(%err, "Error encountered in Service#shutdown");
                        break;
                    }
                },
                ChannelMessage::FSEvent(event) => self.inner.fsevent(&self.info, event).await,
                ChannelMessage::ExternalMessage(msg, sessions) => self.info.send(msg, sessions),
            };

            if let Err(err) = result {
                error!(%err, "Error encountered in service");
            }
        }
    }
}

// Private functions
impl Channel {
    async fn message(&mut self, message: goval::Command, session: SessionID) -> Result<()> {
        if let Some(mut msg) = self
            .inner
            .message(&self.info, message.clone(), session)
            .await?
        {
            msg.r#ref = message.r#ref;
            self.info.send(msg, SendSessions::Only(session))?;
        }

        Ok(())
    }

    async fn attach(
        &mut self,
        session: SessionID,
        client: ClientInfo,
        sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<()> {
        self.info.sessions.insert(session, client.clone());
        self.info.clients.insert(session, sender.clone());
        match self
            .inner
            .attach(&self.info, client, session, sender)
            .await?
        {
            None => {}
            Some(msg) => {
                self.info.send(msg, SendSessions::Only(session))?;
            }
        }
        Ok(())
    }

    async fn detach(&mut self, session: SessionID) -> Result<()> {
        self.info.sessions.retain(|sess, _| sess != &session);
        self.info.clients.retain(|sess, _| sess != &session);
        self.inner.detach(&self.info, session).await?;
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
    "audio",
    "output",
    "shell",
    "toolchain",
    "dotreplit",
    "exec",
    "fsevents",
];

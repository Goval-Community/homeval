mod chat;
mod gcsfiles;
mod ot;
mod presence;
mod traits;
mod types;

use anyhow::format_err;
use anyhow::Result;
use log::as_display;
use log::error;
use std::collections::HashMap;
pub use types::*;

enum LoopControl {
    Cont,
    Break,
}

pub struct Channel {
    info: ChannelInfo,
    _inner: Box<dyn traits::Service + Send>,
}

// Public functions
impl Channel {
    pub fn new(id: i32, service: String, name: Option<String>) -> Result<Channel> {
        let channel: Box<dyn traits::Service + Send> = match service.as_str() {
            "chat" => Box::new(chat::Chat {}),
            "gcsfiles" => Box::new(gcsfiles::GCSFiles {}),
            "presence" => Box::new(presence::Presence::new()),
            "ot" => Box::new(ot::OT::new()),
            _ => return Err(format_err!("Unknown service: {}", service)),
        };

        Ok(Channel {
            info: ChannelInfo {
                id,
                name,
                service,
                clients: HashMap::new(),
                sessions: HashMap::new(),
            },
            _inner: channel,
        })
    }

    pub async fn start(&mut self, mut read: tokio::sync::mpsc::UnboundedReceiver<ChannelMessage>) {
        'mainloop: while let Some(message) = read.recv().await {
            match self.msg(message).await {
                Ok(ctrl) => match ctrl {
                    LoopControl::Break => break 'mainloop,
                    LoopControl::Cont => {}
                },
                Err(err) => {
                    error!(error = as_display!(err); "Error encountered in service")
                }
            }
        }
    }
}

// Private functions
impl Channel {
    async fn msg(&mut self, message: ChannelMessage) -> Result<LoopControl> {
        match message {
            ChannelMessage::Attach(session, client, sender) => {
                self.attach(session, client, sender).await
            }
            ChannelMessage::Detach(session) => self.detach(session).await,
            ChannelMessage::IPC(ipc) => self.message(ipc.command, ipc.session).await,
            ChannelMessage::ProcessDead(_, _) => todo!(),
            ChannelMessage::CmdDead(_) => todo!(),
            ChannelMessage::Replspace(_, _) => todo!(),
            ChannelMessage::Shutdown => {
                self._inner.shutdown(&self.info).await?;
                Ok(LoopControl::Break)
            }
        }
    }

    async fn message(&mut self, message: goval::Command, session: i32) -> Result<LoopControl> {
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
        Ok(LoopControl::Cont)
    }

    async fn attach(
        &mut self,
        session: i32,
        client: ClientInfo,
        sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<LoopControl> {
        self.info.sessions.insert(session, client.clone());
        self.info.clients.insert(session, sender);
        match self._inner.attach(&self.info, client, session).await? {
            None => {}
            Some(msg) => {
                self.info.send(msg, SendSessions::Only(session)).await?;
            }
        }
        Ok(LoopControl::Cont)
    }

    async fn detach(&mut self, session: i32) -> Result<LoopControl> {
        self.info.sessions.retain(|sess, _| sess != &session);
        self.info.clients.retain(|sess, _| sess != &session);
        self._inner.detach(&self.info, session).await?;
        Ok(LoopControl::Cont)
    }
}

pub static IMPLEMENTED_SERVICES: &[&str] = &["chat", "gcsfiles", "presence", "ot"];

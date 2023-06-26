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

pub struct Channel {
    info: ChannelInfo,
    _inner: Box<dyn traits::Service + Send>,
}

// Public functions
impl Channel {
    pub async fn new(id: i32, service: String, name: Option<String>) -> Result<Channel> {
        let channel: Box<dyn traits::Service + Send> = match service.as_str() {
            "chat" => Box::new(chat::Chat {}),
            "gcsfiles" => Box::new(gcsfiles::GCSFiles {}),
            "presence" => Box::new(presence::Presence::new()),
            "ot" => Box::new(ot::OT::new().await?),
            _ => return Err(format_err!("Unknown service: {}", service)),
        };

        let info = ChannelInfo {
            id,
            name,
            service,
            clients: HashMap::new(),
            sessions: HashMap::new(),
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
                ChannelMessage::ProcessDead(_, _) => todo!(),
                ChannelMessage::CmdDead(_) => todo!(),
                ChannelMessage::Replspace(_, _) => todo!(),
                ChannelMessage::Shutdown => match self._inner.shutdown(&self.info).await {
                    Ok(_) => break,
                    Err(err) => {
                        error!(error = as_display!(err); "Error encountered in Service#shutdown");
                        break;
                    }
                },
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

pub static IMPLEMENTED_SERVICES: &[&str] = &["chat", "gcsfiles", "presence", "ot"];

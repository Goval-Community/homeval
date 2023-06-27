pub struct Chat {
    history: Vec<goval::ChatMessage>,
}

use crate::{ClientInfo, IPCMessage, SendSessions};

use super::traits;
use anyhow::{format_err, Result};
use async_trait::async_trait;
use log::{as_debug, warn};

impl Chat {
    pub fn new() -> Chat {
        Chat { history: vec![] }
    }
}

#[async_trait]
impl traits::Service for Chat {
    async fn open(&mut self, _info: &super::types::ChannelInfo) -> Result<()> {
        Ok(())
    }

    async fn message(
        &mut self,
        info: &super::types::ChannelInfo,
        message: goval::Command,
        session: i32,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };

        match body {
            goval::command::Body::ChatMessage(msg) => {
                info.send(message, SendSessions::EveryoneExcept(session))
                    .await?;
                self.history.push(msg);
                Ok(None)
            }
            goval::command::Body::ChatTyping(_) => {
                info.send(message, SendSessions::EveryoneExcept(session))
                    .await?;
                Ok(None)
            }
            _ => {
                warn!(cmd = as_debug!(message); "Unknown chat command");
                Ok(None)
            }
        }
    }

    async fn attach(
        &mut self,
        _info: &super::types::ChannelInfo,
        _client: ClientInfo,
        _session: i32,
        _sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<Option<goval::Command>> {
        let mut scrollback = goval::Command::default();
        let _inner = goval::ChatScrollback {
            scrollback: self.history.clone(),
        };

        scrollback.body = Some(goval::command::Body::ChatScrollback(_inner));

        Ok(Some(scrollback))
    }
}

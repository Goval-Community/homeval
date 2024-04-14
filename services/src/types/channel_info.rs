use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::error;

use crate::config::dotreplit::DotReplit;
use crate::{ChannelID, SessionID};

use super::client::ClientInfo;
use super::messaging::IPCMessage;

#[derive(Clone, Copy, Debug)]
pub enum SendSessions {
    Only(SessionID),
    EveryoneExcept(SessionID),
    Everyone,
}

pub struct ChannelInfo {
    pub id: ChannelID,
    pub clients: HashMap<SessionID, tokio::sync::mpsc::UnboundedSender<IPCMessage>>,
    pub service: String,
    pub name: Option<String>,
    pub sessions: HashMap<SessionID, ClientInfo>,
    pub sender: tokio::sync::mpsc::UnboundedSender<super::ChannelMessage>,
    pub dotreplit: Arc<RwLock<DotReplit>>,
    pub child_env_vars: Arc<RwLock<HashMap<String, String>>>,
}

impl ChannelInfo {
    pub async fn send(&self, mut message: goval::Command, sessions: SendSessions) -> Result<()> {
        let clients: Vec<SessionID>;
        message.channel = self.id.into();
        match sessions {
            SendSessions::Everyone => {
                message.session = 0;
                let mut _clients = vec![];
                for client in self.clients.keys() {
                    _clients.push(*client)
                }

                clients = _clients;
            }
            SendSessions::EveryoneExcept(excluded) => {
                message.session = (-excluded).into();
                let mut _clients = vec![];
                for client in self.clients.keys() {
                    if client != &excluded {
                        _clients.push(*client)
                    }
                }

                clients = _clients;
            }
            SendSessions::Only(session) => {
                message.session = session.into();
                clients = vec![session]
            }
        }

        for client in clients {
            if let Some(sender) = self.clients.get(&client) {
                sender.send(IPCMessage {
                    command: message.clone(),
                    session: client,
                })?;
            } else {
                error!("Missing session outbound message queue in op_send_msg")
            }
        }
        Ok(())
    }
}

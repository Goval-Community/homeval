use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::error;

use crate::config::dotreplit::DotReplit;
use crate::Session;

use super::client::ClientInfo;
use super::messaging::IPCMessage;

#[derive(Clone, Copy, Debug)]
pub enum SendSessions {
    Only(Session),
    EveryoneExcept(Session),
    Everyone,
}

pub struct ChannelInfo {
    pub id: i32,
    pub clients: HashMap<Session, tokio::sync::mpsc::UnboundedSender<IPCMessage>>,
    pub service: String,
    pub name: Option<String>,
    pub sessions: HashMap<Session, ClientInfo>,
    pub sender: tokio::sync::mpsc::UnboundedSender<super::ChannelMessage>,
    pub dotreplit: Arc<RwLock<DotReplit>>,
    pub child_env_vars: Arc<RwLock<HashMap<String, String>>>,
}

impl ChannelInfo {
    pub async fn send(&self, mut message: goval::Command, sessions: SendSessions) -> Result<()> {
        let clients: Vec<Session>;
        message.channel = self.id;
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

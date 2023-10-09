use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use goval;
use log::error;
use tokio::sync::RwLock;

use crate::config::dotreplit::DotReplit;

use super::client::ClientInfo;
use super::messaging::IPCMessage;

pub enum SendSessions {
    Only(i32),
    EveryoneExcept(i32),
    Everyone,
}

pub struct ChannelInfo {
    pub id: i32,
    pub clients: HashMap<i32, tokio::sync::mpsc::UnboundedSender<IPCMessage>>,
    pub service: String,
    pub name: Option<String>,
    pub sessions: HashMap<i32, ClientInfo>,
    pub sender: tokio::sync::mpsc::UnboundedSender<super::ChannelMessage>,
    pub dotreplit: Arc<RwLock<DotReplit>>,
}

impl ChannelInfo {
    pub async fn send(&self, mut message: goval::Command, sessions: SendSessions) -> Result<()> {
        let clients: Vec<i32>;
        message.channel = self.id;
        match sessions {
            SendSessions::Everyone => {
                message.session = 0;
                let mut _clients = vec![];
                for client in self.clients.keys() {
                    _clients.push(client.clone())
                }

                clients = _clients;
            }
            SendSessions::EveryoneExcept(excluded) => {
                message.session = -excluded;
                let mut _clients = vec![];
                for client in self.clients.keys() {
                    if client != &excluded {
                        _clients.push(client.clone())
                    }
                }

                clients = _clients;
            }
            SendSessions::Only(session) => {
                message.session = session;
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

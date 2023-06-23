use std::collections::HashMap;

use anyhow::Result;
use goval;
use log::error;
use prost::Message;
use serde;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub enum SendSessions {
    Only(i32),
    EveryoneExcept(i32),
    Everyone,
}

pub struct ChannelInfo {
    pub id: i32,
    pub clients: HashMap<i32, mpsc::UnboundedSender<IPCMessage>>,
    pub service: String,
    pub name: Option<String>,
    pub sessions: HashMap<i32, ClientInfo>,
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

#[derive(Clone)]
pub struct ServiceMetadata {
    pub service: String,
    pub name: Option<String>,
    pub id: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ReplspaceMessage {
    GithubTokenReq(String),                 // nonce
    OpenFileReq(String, bool, String),      // file, wait for close, nonce
    OpenMultipleFiles(Vec<String>, String), // files, nonce

    GithubTokenRes(String), // token
    OpenFileRes,
}

#[derive(Clone, Debug)]
pub enum ChannelMessage {
    IPC(IPCMessage),
    Attach(i32, ClientInfo, mpsc::UnboundedSender<IPCMessage>),
    Detach(i32),
    ProcessDead(u32, i32),
    CmdDead(i32),
    Replspace(i32, ReplspaceMessage), // session, message
    Shutdown, // Shutdown the service, value has to be true so that runtime.js can match it in an if check
}

#[derive(Debug, Clone)]
pub struct IPCMessage {
    pub command: goval::Command,
    pub session: i32,
}

impl IPCMessage {
    pub fn replace_cmd(&self, cmd: goval::Command) -> IPCMessage {
        IPCMessage {
            command: cmd,
            session: self.session,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.command.encode_to_vec()
    }
}

impl TryFrom<Vec<u8>> for IPCMessage {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(Self {
            command: goval::Command::decode(value.as_slice())?,
            session: 0,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ClientInfo {
    pub is_secure: bool,

    pub username: String,
    pub id: u32,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            is_secure: false,

            username: "homeval-user".to_owned(),
            id: 23054564,
        }
    }
}

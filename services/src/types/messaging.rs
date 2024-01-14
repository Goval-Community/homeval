use anyhow::Result;
use goval;
use prost::Message;
use serde;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

use crate::SendSessions;

use super::client::ClientInfo;

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
    Attach(
        i32,
        ClientInfo,
        tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ),
    Detach(i32),
    ProcessDead(i32),
    FSEvent(super::FSEvent),
    Replspace(i32, ReplspaceMessage, Option<Sender<ReplspaceMessage>>), // session, message
    Shutdown,
    ExternalMessage(goval::Command, SendSessions),
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

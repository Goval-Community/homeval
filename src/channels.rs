use crate::goval;
use prost::Message;
use serde;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IPCMessage {
    pub bytes: Vec<u8>,
    pub session: i32,
}

impl IPCMessage {
    pub fn from_cmd(cmd: goval::Command, session: i32) -> IPCMessage {
        let mut bytes = Vec::new();
        bytes.reserve(cmd.encoded_len());
        cmd.encode(&mut bytes).unwrap();

        IPCMessage { bytes, session }
    }

    pub fn to_cmd(&self) -> Result<goval::Command, prost::DecodeError> {
        Ok(goval::Command::decode(&*self.bytes)?)
    }

    pub fn replace_cmd(&self, cmd: goval::Command) -> IPCMessage {
        IPCMessage::from_cmd(cmd, self.session)
    }
}

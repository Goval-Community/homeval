use crate::goval;
use base64::{engine::general_purpose::STANDARD as base64, Engine as _};
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
    #[allow(dead_code)]
    pub fn to_cmd(&self) -> Result<goval::Command, prost::DecodeError> {
        Ok(goval::Command::decode(&*self.bytes)?)
    }

    pub fn to_js(&self) -> JsMessage {
        JsMessage {
            contents: base64.encode(&*self.bytes),
            session: self.session,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsMessage {
    pub contents: String,
    pub session: i32,
}

impl JsMessage {
    pub fn to_ipc(&self) -> IPCMessage {
        IPCMessage {
            bytes: base64.decode(self.contents.clone()).unwrap(),
            session: self.session,
        }
    }
}

use std::io::Error;

use base64::{engine::general_purpose, Engine as _};
use deno_core::error::AnyError;
use homeval::goval;
use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    // In the future this will indicate if the jwt signature was legit or not
    // For now it'll always be false
    pub is_secure: bool,

    pub username: String,
    pub id: u32,
}

impl ClientInfo {
    pub fn default() -> ClientInfo {
        ClientInfo {
            is_secure: false,

            username: "homeval-user".to_owned(),
            id: 23054564,
        }
    }
}

pub fn parse(token: &str) -> Result<ClientInfo, AnyError> {
    let token_parts = token.split(".").collect::<Vec<_>>();
    if token_parts.len() < 3 {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid Token",
        )));
    }

    if token_parts[0] != "v2" || token_parts[1] != "public" {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid Token",
        )));
    }

    let decoded = general_purpose::URL_SAFE_NO_PAD.decode(token_parts[2].as_bytes())?;
    let decoded_len = decoded.len();
    // currently doesn't verify signature
    let (msg, _sig) = decoded.split_at(decoded_len - 64);

    let _inner = general_purpose::STANDARD.decode(msg)?;
    let inner = goval::ReplToken::decode(_inner.as_slice())?;

    match inner.presenced {
        Some(user) => Ok(ClientInfo {
            is_secure: false,

            username: user.bearer_name,
            id: user.bearer_id,
        }),
        None => Ok(ClientInfo::default()),
    }
}

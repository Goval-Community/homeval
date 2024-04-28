pub struct GovalIdent {}

use crate::{server_info::ServerInfoSerialize, ClientInfo, IPCMessage, SessionID};

use super::traits;
use anyhow::{format_err, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

#[derive(Deserialize)]
enum IdentReqType {
    ServerInfo,

    #[serde(untagged)]
    Unknown(String),
}

#[derive(Deserialize)]
struct IdentRequest {
    r#type: IdentReqType,
    r#ref: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum IdentResponseEnum<'a> {
    Error(String),
    Data(ServerInfoSerialize<'a>),
}

#[derive(Serialize)]
struct IdentResponse<'a> {
    body: IdentResponseEnum<'a>,
    r#ref: String,
}

#[async_trait]
impl traits::Service for GovalIdent {
    async fn message(
        &mut self,
        info: &super::types::ChannelInfo,
        message: goval::Command,
        session: SessionID,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };

        match body {
            goval::command::Body::ChatMessage(msg) => {
                let req: IdentRequest = serde_json::from_str(&msg.text)?;

                match req.r#type {
                    IdentReqType::ServerInfo => {
                        let msg_text = serde_json::to_string(&IdentResponse {
                            body: IdentResponseEnum::Data(info.server_info.get_serializable()),
                            r#ref: req.r#ref,
                        })?;

                        let response_body = goval::command::Body::ChatMessage(goval::ChatMessage {
                            username: "goval".to_string(),
                            text: msg_text,
                        });

                        let response = goval::Command {
                            body: Some(response_body),
                            ..Default::default()
                        };

                        Ok(Some(response))
                    }
                    IdentReqType::Unknown(req_type) => {
                        error!(req_type, "Unknown `goval-ident` request type");

                        let msg_text = serde_json::to_string(&IdentResponse {
                            body: IdentResponseEnum::Error(format!(
                                "Unknown request type {req_type}"
                            )),
                            r#ref: req.r#ref,
                        })?;

                        let response_body = goval::command::Body::ChatMessage(goval::ChatMessage {
                            username: "goval".to_string(),
                            text: msg_text,
                        });

                        let response = goval::Command {
                            body: Some(response_body),
                            ..Default::default()
                        };

                        Ok(Some(response))
                    }
                }
            }
            goval::command::Body::ChatTyping(typing) => {
                warn!(
                    %session,
                    username = typing.username,
                    "Chat#ChatTyping isn't supported by `goval-ident`",
                );
                Ok(None)
            }
            _ => {
                warn!(cmd = ?message, "Unknown goval-ident command");
                Ok(None)
            }
        }
    }

    async fn attach(
        &mut self,
        _info: &super::types::ChannelInfo,
        _client: ClientInfo,
        _session: SessionID,
        _sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<Option<goval::Command>> {
        let mut scrollback = goval::Command::default();

        let _inner = goval::ChatMessage {
            username: "goval".to_string(),
            text: "{\"type\": \"notification\"}".to_string(),
        };

        scrollback.body = Some(goval::command::Body::ChatMessage(_inner));

        Ok(Some(scrollback))
    }
}

pub struct Git {
    replspace: HashMap<String, Option<Sender<ReplspaceMessage>>>,
}
use std::collections::HashMap;

use crate::ReplspaceMessage;

use super::traits;
use anyhow::{format_err, Result};
use async_trait::async_trait;
use log::{as_debug, warn};
use tokio::sync::mpsc::Sender;

#[async_trait]
impl traits::Service for Git {
    async fn message(
        &mut self,
        _info: &super::types::ChannelInfo,
        message: goval::Command,
        _session: i32,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };

        match body {
            goval::command::Body::ReplspaceApiGitHubToken(token) => {
                match self.replspace.get(&token.nonce) {
                    Some(_respond) => {
                        if let Some(respond) = _respond {
                            respond
                                .send(ReplspaceMessage::GithubTokenRes(token.token))
                                .await?;
                        }
                    }
                    None => {
                        warn!(msg = as_debug!(message), nonce = token.nonce; "Missing replspace response callback for github token");
                    }
                }
            }
            goval::command::Body::ReplspaceApiCloseFile(close) => {
                match self.replspace.get(&close.nonce) {
                    Some(_respond) => {
                        if let Some(respond) = _respond {
                            respond.send(ReplspaceMessage::OpenFileRes).await?;
                        }
                    }
                    None => {
                        warn!(msg = as_debug!(message), nonce = close.nonce; "Missing replspace response callback for close file");
                    }
                }
            }
            _ => {}
        }

        Ok(None)
    }

    async fn replspace(
        &mut self,
        info: &super::types::ChannelInfo,
        msg: ReplspaceMessage,
        session: i32,
        respond: Option<Sender<ReplspaceMessage>>,
    ) -> Result<()> {
        if session == 0 {
            warn!(msg = as_debug!(msg); "Got replspace message from an unknown session, ignoring");
            return Ok(());
        }

        match msg {
            ReplspaceMessage::GithubTokenReq(nonce) => {
                let token_req = goval::Command {
                    body: Some(goval::command::Body::ReplspaceApiGetGitHubToken(
                        goval::ReplspaceApiGetGitHubToken {
                            nonce: nonce.clone(),
                        },
                    )),
                    ..Default::default()
                };
                info.send(token_req, crate::SendSessions::Only(session))
                    .await?;

                self.replspace.insert(nonce, respond);
            }
            ReplspaceMessage::OpenFileReq(path, wait_for_close, nonce) => {
                let token_req = goval::Command {
                    body: Some(goval::command::Body::ReplspaceApiOpenFile(
                        goval::ReplspaceApiOpenFile {
                            nonce: nonce.clone(),
                            wait_for_close,
                            file: path,
                        },
                    )),
                    ..Default::default()
                };
                info.send(token_req, crate::SendSessions::Only(session))
                    .await?;

                self.replspace.insert(nonce, respond);
            }
            _ => {}
        }

        Ok(())
    }
}

impl Git {
    pub fn new() -> Git {
        Git {
            replspace: HashMap::new(),
        }
    }
}

pub struct Shell {
    pty: Pty,
}
use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::debug;

use super::traits;
use super::types::pty::Pty;
use crate::{ClientInfo, IPCMessage};
use anyhow::{format_err, Result};

#[async_trait]
impl traits::Service for Shell {
    async fn attach(
        &mut self,
        _info: &super::types::ChannelInfo,
        _client: ClientInfo,
        session: i32,
        sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<Option<goval::Command>> {
        self.pty.session_join(session, sender).await?;
        Ok(None)
    }

    async fn detach(&mut self, _info: &super::types::ChannelInfo, session: i32) -> Result<()> {
        self.pty.session_leave(session).await?;
        Ok(())
    }

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
            goval::command::Body::Input(msg) => {
                self.pty.write(msg)?;
            }
            goval::command::Body::ResizeTerm(size) => {
                self.pty.resize(size.rows as u16, size.cols as u16)?
            }
            _ => {
                debug!(?message, "New message");
            }
        }
        Ok(None)
    }

    async fn proccess_died(
        &mut self,
        info: &super::types::ChannelInfo,
        _exit_code: i32,
    ) -> Result<()> {
        self.pty = Shell::start_pty(info).await?;
        Ok(())
    }
}

#[cfg(target_family = "unix")]
static DEFAULT_SHELL: &str = "sh";
#[cfg(target_family = "windows")]
static DEFAULT_SHELL: &str = "pwsh";

impl Shell {
    async fn start_pty(info: &super::types::ChannelInfo) -> Result<Pty> {
        let mut env = HashMap::new();
        env.insert("REPLIT_GIT_TOOLS_CHANNEL_FROM".into(), info.id.to_string());
        Pty::start(
            vec![std::env::var("SHELL").unwrap_or(DEFAULT_SHELL.to_string())],
            info.id,
            Arc::new(RwLock::new(info.clients.clone())),
            info.sender.clone(),
            Some(env),
        )
        .await
    }
    pub async fn new(info: &super::types::ChannelInfo) -> Result<Shell> {
        Ok(Shell {
            pty: Shell::start_pty(info).await?,
        })
    }
}

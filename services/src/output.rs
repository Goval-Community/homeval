pub struct Output {
    pty: Option<Pty>,
    start_time: Option<i64>,
}
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
    vec,
};

use async_trait::async_trait;
use prost_types::Timestamp;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::traits;
use super::types::pty::Pty;
use crate::{ClientInfo, IPCMessage};
use anyhow::{format_err, Result};

#[async_trait]
impl traits::Service for Output {
    async fn attach(
        &mut self,
        info: &super::types::ChannelInfo,
        _client: ClientInfo,
        session: i32,
        sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<Option<goval::Command>> {
        if let Some(pty) = &mut self.pty {
            let mut new_frame = goval::Command::default();

            let event = goval::OutputBlockStartEvent {
                execution_mode: goval::output_block_start_event::ExecutionMode::Run.into(),
                measure_start_time: Some(Timestamp {
                    seconds: self.start_time.unwrap_or(0),
                    nanos: 0,
                }),
            };

            new_frame.body = Some(goval::command::Body::OutputBlockStartEvent(event));
            info.send(new_frame, crate::SendSessions::Only(session))
                .await?;

            pty.session_join(session, sender).await?;
        }

        let mut status = goval::Command::default();
        let state = if self.start_time.is_some() {
            goval::State::Running
        } else {
            goval::State::Stopped
        };

        status.body = Some(goval::command::Body::State(state.into()));
        Ok(Some(status))
    }

    async fn detach(&mut self, _info: &super::types::ChannelInfo, session: i32) -> Result<()> {
        if let Some(pty) = &mut self.pty {
            pty.session_leave(session).await?;
        }
        Ok(())
    }

    async fn message(
        &mut self,
        info: &super::types::ChannelInfo,
        message: goval::Command,
        _session: i32,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };

        match body {
            goval::command::Body::RunMain(_) => {
                let time = SystemTime::now();
                let now = time.duration_since(UNIX_EPOCH)?.as_secs() as i64;
                self.start_time = Some(now);
                let mut cmd = vec![
                    "echo".to_string(),
                    "Please configure a run command in `.replit`".to_string(),
                ];
                if let Some(run) = &info.dotreplit.read().await.run {
                    if let Some(args) = &run.args {
                        cmd = args.clone()
                    }
                }

                let mut env = HashMap::new();
                env.insert("REPLIT_GIT_TOOLS_CHANNEL_FROM".into(), info.id.to_string());

                self.pty = Some(
                    Pty::start(
                        cmd,
                        info.id,
                        Arc::new(RwLock::new(info.clients.clone())),
                        info.sender.clone(),
                        Some(env),
                    )
                    .await?,
                );

                let mut new_frame = goval::Command::default();

                let event = goval::OutputBlockStartEvent {
                    execution_mode: goval::output_block_start_event::ExecutionMode::Run.into(),
                    measure_start_time: Some(Timestamp {
                        seconds: now,
                        nanos: 0,
                    }),
                };

                new_frame.body = Some(goval::command::Body::OutputBlockStartEvent(event));
                info.send(new_frame, crate::SendSessions::Everyone).await?;

                let status = goval::Command {
                    body: Some(goval::command::Body::State(goval::State::Running.into())),
                    ..Default::default()
                };

                info.send(status, crate::SendSessions::Everyone).await?;
            }
            goval::command::Body::Clear(_) => {
                if let Some(pty) = &mut self.pty {
                    pty.cancel().await?;
                } else {
                    warn!("Client tried to stop an already stopped pty")
                }
            }
            goval::command::Body::Input(msg) => {
                if let Some(pty) = &mut self.pty {
                    pty.write(msg)?;
                }
            }
            goval::command::Body::ResizeTerm(_) => {}
            _ => {
                debug!(?message, "New message");
            }
        }
        Ok(None)
    }

    async fn proccess_died(
        &mut self,
        info: &super::types::ChannelInfo,
        exit_code: i32,
    ) -> Result<()> {
        self.pty = None;
        self.start_time = None;

        let time = SystemTime::now();
        let now = time.duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let mut end_frame = goval::Command::default();

        let event = goval::OutputBlockEndEvent {
            exit_code,
            measure_end_time: Some(Timestamp {
                seconds: now,
                nanos: 0,
            }),
        };

        end_frame.body = Some(goval::command::Body::OutputBlockEndEvent(event));
        info.send(end_frame, crate::SendSessions::Everyone).await?;

        if exit_code != 0 {
            let error = goval::Command {
                body: Some(goval::command::Body::Error(format!(
                    "exit code {exit_code}"
                ))),
                ..Default::default()
            };

            info.send(error, crate::SendSessions::Everyone).await?;
        }

        let status = goval::Command {
            body: Some(goval::command::Body::State(goval::State::Stopped.into())),
            ..Default::default()
        };
        info.send(status, crate::SendSessions::Everyone).await?;
        Ok(())
    }
}

impl Output {
    pub async fn new() -> Output {
        Output {
            pty: None,
            start_time: None,
        }
    }
}

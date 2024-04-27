pub struct Exec {
    running: bool,
    queue: Vec<(goval::Exec, String)>,
    current_ref: String,
}

use crate::{Proc, SessionID};

use super::traits;
use anyhow::{format_err, Result};
use async_trait::async_trait;

#[async_trait]
impl traits::Service for Exec {
    async fn message(
        &mut self,
        info: &super::types::ChannelInfo,
        message: goval::Command,
        _session: SessionID,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };

        if let goval::command::Body::Exec(exec) = body {
            if self.running {
                if !(exec.blocking || exec.lifecycle == goval::exec::Lifecycle::Blocking as i32) {
                    info.send(
                        goval::Command {
                            body: Some(goval::command::Body::Error("Already running".to_string())),
                            ..Default::default()
                        },
                        crate::SendSessions::Everyone,
                    )?;
                    return Ok(None);
                }

                self.queue.push((exec, message.r#ref));
            } else {
                info.send(
                    goval::Command {
                        body: Some(goval::command::Body::State(goval::State::Stopped.into())),
                        ..Default::default()
                    },
                    crate::SendSessions::Everyone,
                )?;
                self.running = true;
                self.current_ref = message.r#ref;
                Proc::new(exec.args, info.id, info.sender.clone(), Some(&exec.env))?;
                info.send(
                    goval::Command {
                        body: Some(goval::command::Body::State(goval::State::Running.into())),
                        ..Default::default()
                    },
                    crate::SendSessions::Everyone,
                )?;
            }
        }

        Ok(None)
    }

    async fn proccess_died(
        &mut self,
        info: &super::types::ChannelInfo,
        exit_code: i32,
    ) -> Result<()> {
        self.running = false;
        if exit_code == 0 {
            info.send(
                goval::Command {
                    body: Some(goval::command::Body::Ok(goval::Ok {})),
                    r#ref: self.current_ref.clone(),
                    ..Default::default()
                },
                crate::SendSessions::Everyone,
            )?;
        } else {
            info.send(
                goval::Command {
                    body: Some(goval::command::Body::Error(format!(
                        "exit status {exit_code}"
                    ))),
                    r#ref: self.current_ref.clone(),
                    ..Default::default()
                },
                crate::SendSessions::Everyone,
            )?;
        }

        self.current_ref = String::new();

        info.send(
            goval::Command {
                body: Some(goval::command::Body::State(goval::State::Stopped.into())),
                ..Default::default()
            },
            crate::SendSessions::Everyone,
        )?;

        if !self.queue.is_empty() {
            self.running = true;
            let item = self.queue.swap_remove(0);
            Proc::new(item.0.args, info.id, info.sender.clone(), Some(&item.0.env))?;
            self.current_ref = item.1;
            info.send(
                goval::Command {
                    body: Some(goval::command::Body::State(goval::State::Running.into())),
                    ..Default::default()
                },
                crate::SendSessions::Everyone,
            )?;
        }

        Ok(())
    }
}

impl Exec {
    pub fn new() -> Self {
        Exec {
            running: false,
            queue: vec![],
            current_ref: String::new(),
        }
    }
}

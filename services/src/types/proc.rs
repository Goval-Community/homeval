use std::{
    collections::{HashMap, VecDeque},
    pin::Pin,
    process::{ExitStatus, Stdio},
    sync::{atomic::AtomicBool, Arc},
    task::{Context, Poll},
};

use crate::{ChannelMessage, IPCMessage, SendSessions};
use anyhow::Result;
use log::{error, trace};
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    process::ChildStdin,
    sync::RwLock,
};

struct CmdWriter {
    channel: i32,
    contact: tokio::sync::mpsc::UnboundedSender<super::ChannelMessage>,
    cancelled: Arc<AtomicBool>,
    error: bool,
}

impl AsyncWrite for CmdWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "cancelled",
            )));
        }
        let mut cmd = goval::Command::default();
        let output = match String::from_utf8(buf.to_vec()) {
            Ok(str) => str,
            Err(err) => {
                error!("Invalid utf-8 output in pty handler");

                return Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    err.utf8_error(),
                )));
            }
        };

        if self.error {
            cmd.body = Some(goval::command::Body::Error(output));
        } else {
            cmd.body = Some(goval::command::Body::Output(output));
        }

        cmd.channel = self.channel;
        if self
            .contact
            .send(ChannelMessage::ExternalMessage(cmd, SendSessions::Everyone))
            .is_err()
        {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Proc recv'ing channel was dropped",
            )));
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
}

pub struct Proc {
    channel: i32,
    cancelled: Arc<AtomicBool>,
    contact: tokio::sync::mpsc::UnboundedSender<super::ChannelMessage>,
    stdin: ChildStdin,
}

impl Proc {
    pub async fn new(
        _args: Vec<String>,
        channel: i32,
        contact: tokio::sync::mpsc::UnboundedSender<super::ChannelMessage>,
        _env: Option<HashMap<String, String>>,
    ) -> Result<Self> {
        let cancelled = Arc::new(AtomicBool::new(false));

        let mut cmd = tokio::process::Command::new(&_args[0]);
        let args = &mut VecDeque::from(_args.to_vec());
        trace!("{:#?}", args);
        VecDeque::pop_front(args);
        for arg in args {
            cmd.arg(arg);
        }
        // debug!("{:#?}", std::env::current_dir()?);
        cmd.current_dir(std::env::current_dir()?);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::piped());
        let mut child = cmd.spawn()?;
        let mut stdout_opt = child.stdout.take().expect("TODO: handle this");
        let mut stderr_opt = child.stderr.take().expect("TODO: handle this");
        let stdin = child.stdin.take().expect("TODO: handle this");

        let contact_clone = contact.clone();
        let cancelled_clone = cancelled.clone();
        tokio::task::spawn(async move {
            let mut sender = CmdWriter {
                channel,
                contact: contact_clone,
                cancelled: cancelled_clone,
                error: false,
            };
            tokio::io::copy(&mut stdout_opt, &mut sender)
                .await
                .expect("TODO: handle this");
        });

        let contact_clone = contact.clone();
        let cancelled_clone = cancelled.clone();
        tokio::task::spawn(async move {
            let mut sender = CmdWriter {
                channel,
                contact: contact_clone,
                cancelled: cancelled_clone,
                error: true,
            };
            tokio::io::copy(&mut stderr_opt, &mut sender)
                .await
                .expect("TODO: handle this");
        });

        let contact_clone = contact.clone();
        let cancelled_clone = cancelled.clone();
        tokio::task::spawn(async move {
            let exit_status: i32;
            loop {
                if let Some(exit_code) = child.try_wait().expect("TODO: handle this") {
                    // TODO: is defaulting to -1 correct?
                    // #[cfg(target_family = "unix")]
                    // exit_status = std::os::unix::process::ExitStatusExt::into_raw(exit_code);
                    // #[cfg(not(target_family = "unix"))]
                    exit_status = exit_code.code().unwrap_or(-1);
                    break;
                }

                if cancelled_clone.load(std::sync::atomic::Ordering::SeqCst) {
                    child.kill().await.expect("TODO: handle this");
                    exit_status = -1;
                    break;
                }

                // Yield to not block event loop with busy loop
                tokio::task::yield_now().await;
            }

            if contact_clone
                .send(ChannelMessage::ProcessDead(exit_status))
                .is_err()
            {
                error!("Proc recv'ing channel was dropped before process dead alert was sent")
            }
        });

        Ok(Proc {
            channel,
            contact,
            cancelled,
            stdin,
        })
    }

    pub fn cancel(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub async fn write(&mut self, src: &[u8]) -> Result<()> {
        Ok(self.stdin.write_all(src).await?)
    }
}

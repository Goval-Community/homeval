// use futures_util::{future::abortable, stream::AbortHandle};
use log::{as_display, as_error, error};
use portable_pty::{Child, PtyPair, PtySize};
use std::{
    collections::{HashMap, VecDeque},
    io::{Error, ErrorKind, Write},
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use crate::ChannelMessage;

use super::IPCMessage;

use anyhow::{format_err, Result};
use tokio::sync::{Mutex, RwLock};
// use deno_core::{error::AnyError, op, OpDecl};

// static PTY_CANCELLATION_MAP: LazyLock<RwLock<HashMap<u32, AbortHandle>>> =
//     LazyLock::new(|| RwLock::new(HashMap::new()));
// static PTY_SESSION_MAP: LazyLock<RwLock<HashMap<u32, Vec<i32>>>> =
//     LazyLock::new(|| RwLock::new(HashMap::new()));

struct PtyWriter {
    channel: i32,
    sessions: Arc<RwLock<HashMap<i32, tokio::sync::mpsc::UnboundedSender<IPCMessage>>>>,
    cancelled: Arc<AtomicBool>,
    scrollback: Arc<RwLock<String>>,
}

static MAX_SCROLLBACK: usize = 10_000;

impl Write for PtyWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut cmd = goval::Command::default();
        let output: String;
        match String::from_utf8(buf.to_vec()) {
            Ok(str) => output = str,
            Err(err) => {
                error!("Invalid utf-8 output in pty handler");

                return Err(Error::new(ErrorKind::Other, err.utf8_error()));
            }
        }

        cmd.body = Some(goval::command::Body::Output(output.clone()));
        cmd.channel = self.channel;

        if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::new(ErrorKind::Other, "cancelled"));
        }

        let mut scrollback = self.scrollback.blocking_write();
        *scrollback += &output;
        let scrollback_len = scrollback.len();
        if scrollback_len > MAX_SCROLLBACK {
            *scrollback = scrollback
                .split_at(scrollback_len - MAX_SCROLLBACK)
                .1
                .to_string();
        }
        drop(scrollback);

        let sessions = self.sessions.blocking_read();
        if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::new(ErrorKind::Other, "cancelled"));
        }

        for (session, sender) in sessions.iter() {
            let mut to_send = cmd.clone();
            to_send.session = *session;

            match sender.send(IPCMessage {
                command: to_send,
                session: *session,
            }) {
                Ok(_) => {}
                Err(err) => {
                    return Err(Error::new(ErrorKind::Other, err));
                }
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct Pty {
    channel: i32,
    pub sessions: Arc<RwLock<HashMap<i32, tokio::sync::mpsc::UnboundedSender<IPCMessage>>>>,
    writer: Box<dyn Write + Send>,
    cancelled: Arc<AtomicBool>,
    child_lock: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    scrollback: Arc<RwLock<String>>,
    pair: PtyPair,
}

impl Pty {
    pub async fn start(
        _args: Vec<String>,
        channel: i32,
        sessions: Arc<RwLock<HashMap<i32, tokio::sync::mpsc::UnboundedSender<IPCMessage>>>>,
        contact: tokio::sync::mpsc::UnboundedSender<super::ChannelMessage>,
        _env: Option<HashMap<String, String>>,
    ) -> Result<Pty> {
        let env = match _env {
            Some(env) => env,
            None => HashMap::new(),
        };

        let pty_system = portable_pty::native_pty_system();

        // Create a new pty
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            // Not all systems support pixel_width, pixel_height,
            // but it is good practice to set it to something
            // that matches the size of the selected font.  That
            // is more complex than can be shown here in this
            // brief example though!
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = portable_pty::CommandBuilder::new(_args[0].clone());
        let args = &mut VecDeque::from(_args.to_vec());
        VecDeque::pop_front(args);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.cwd(std::env::current_dir()?);

        for (key, val) in env.into_iter() {
            cmd.env(key, val)
        }

        let child = pair.slave.spawn_command(cmd)?;

        // TODO: recode checkpoint

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        // let pty_id = child.process_id().expect("Missing process id????");

        let child_lock = Arc::new(Mutex::new(child));

        let cancelled = Arc::new(AtomicBool::new(false));
        let scrollback = Arc::new(RwLock::new(String::new()));
        let mut pty_writer = PtyWriter {
            channel,
            sessions: sessions.clone(),
            cancelled: cancelled.clone(),
            scrollback: scrollback.clone(),
        };

        let contact_clone = contact.clone();
        tokio::task::spawn(async move {
            if let Err(err) =
                tokio::task::spawn_blocking(move || std::io::copy(&mut reader, &mut pty_writer))
                    .await
            {
                error!("Error occurred copying from pty to channels: {}", err);
            };

            // let _read = crate::CHANNEL_MESSAGES.read().await;
            // if !_read.contains_key(&channel) {
            //     return Err(format_err!("Owning channel"));
            // }
        });

        let child_lock_reaper = child_lock.clone();
        tokio::task::spawn(async move {
            match tokio::task::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(50));

                loop {
                    interval.tick().await;
                    let mut child_ = child_lock_reaper.lock().await;
                    if let Some(exit_code) = child_.try_wait()? {
                        return Ok::<i32, anyhow::Error>(exit_code.exit_code() as i32);
                    }
                    drop(child_);
                }
            })
            .await
            {
                Ok(res) => {
                    match res {
                        Ok(exit_code) => {
                            // let queue = _read.get(&channel).unwrap().clone();
                            // drop(_read);
                            match contact_clone.send(ChannelMessage::ProcessDead(exit_code)) {
                                Ok(_) => {}
                                Err(err) => {
                                    error!(err = as_error!(err); "PTY child proc reaper errored when alerting channel")
                                }
                            }
                        }
                        Err(err) => {
                            error!(err = as_display!(err); "PTY child proc reaper errored")
                        }
                    }
                }
                Err(err) => {
                    error!(err = as_error!(err); "Join error on pty child proc reaper")
                }
            }
        });

        // tokio::spawn(async move {
        //     match task.await {
        //         Ok(err) => {
        //             error!("Error occurred while passing writes to pty: {}", err)
        //         }
        //         Err(_) => {
        //             let mut child = child_lock.lock().await;
        //             match child.kill() {
        //                 Ok(_) => {}
        //                 Err(err) => {
        //                     warn!("Failed to kill pty child: {}", err)
        //                 }
        //             }
        //             drop(child);
        //         }
        //     }
        // });

        let pty = Pty {
            channel,
            sessions,
            writer,
            cancelled,
            child_lock,
            scrollback,
            pair,
        };
        Ok(pty)
    }

    // TODO: use spawn_blocking, bcuz interacting with sync code
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.pair.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    pub async fn cancel(&mut self) -> Result<()> {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.child_lock.lock().await.kill()?;
        Ok(())
    }

    pub fn write(&mut self, task: String) -> Result<()> {
        if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(format_err!("Can't write to a cancelled pty"));
        }

        self.writer.write(task.as_bytes())?;
        Ok(())
    }

    pub async fn session_join(
        &mut self,
        session: i32,
        sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<()> {
        if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(format_err!("Can't add a session to a cancelled pty"));
        };
        let mut cmd = goval::Command::default();

        cmd.body = Some(goval::command::Body::Output(
            self.scrollback.read().await.clone(),
        ));
        cmd.session = session;
        cmd.channel = self.channel;

        sender.send(IPCMessage {
            command: cmd,
            session,
        })?;

        self.sessions.write().await.insert(session, sender);

        Ok(())
    }

    pub async fn session_leave(&mut self, session: i32) -> Result<()> {
        if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(format_err!("Can't remove a session from a cancelled pty"));
        }
        self.sessions.write().await.remove(&session);
        Ok(())
    }
}

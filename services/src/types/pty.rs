use futures_util::{future::abortable, stream::AbortHandle};
use log::{error, warn};
use portable_pty::PtySize;
use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    io::{Error, ErrorKind, Write},
    sync::Arc,
};

use super::IPCMessage;

use anyhow::{format_err, Result};
use tokio::sync::{Mutex, RwLock};
// use deno_core::{error::AnyError, op, OpDecl};

use super::ChannelMessage;

// static PTY_CANCELLATION_MAP: LazyLock<RwLock<HashMap<u32, AbortHandle>>> =
//     LazyLock::new(|| RwLock::new(HashMap::new()));
// static PTY_SESSION_MAP: LazyLock<RwLock<HashMap<u32, Vec<i32>>>> =
//     LazyLock::new(|| RwLock::new(HashMap::new()));

struct PtyWriter {
    channel: i32,
    sessions: Arc<RwLock<HashMap<i32, tokio::sync::mpsc::UnboundedSender<IPCMessage>>>>,
}

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

        cmd.body = Some(goval::command::Body::Output(output));
        cmd.channel = self.channel;

        let sessions = self.sessions.blocking_read();

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

struct Pty {
    channel: i32,
    sessions: Arc<RwLock<HashMap<i32, tokio::sync::mpsc::UnboundedSender<IPCMessage>>>>,
}

impl Pty {
    pub async fn start(
        _args: Vec<String>,
        channel: i32,
        sessions: Arc<RwLock<HashMap<i32, tokio::sync::mpsc::UnboundedSender<IPCMessage>>>>,
        _env: Option<HashMap<String, String>>,
    ) -> Result<Pty> {
        let pty = Pty { channel, sessions };
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
        let mut writer = pair.master.take_writer()?;

        let pty_id = child.process_id().expect("Missing process id????");

        let child_lock = Arc::new(Mutex::new(child));

        let child_lock_reaper = child_lock.clone();
        tokio::task::spawn(async move {
            if let Err(err) = tokio::task::spawn_blocking(move || {
                std::io::copy(&mut reader, &mut PtyWriter { channel, sessions })
            })
            .await
            {
                error!("Error occurred copying from pty to channels: {}", err);
            };

            // let _read = crate::CHANNEL_MESSAGES.read().await;
            // if !_read.contains_key(&channel) {
            //     return Err(format_err!("Owning channel"));
            // }

            // let exit_code;

            // if let Some(code) = child_lock_reaper.lock().await.try_wait()? {
            //     exit_code = code.exit_code() as i32;
            // } else {s
            //     exit_code = 0;
            // }

            // let queue = _read.get(&channel).unwrap().clone();
            // drop(_read);
            // queue.push(ChannelMessage::ProcessDead(pty_id, exit_code));
            Ok(())
        });

        tokio::spawn(async move {
            match task.await {
                Ok(err) => {
                    error!("Error occurred while passing writes to pty: {}", err)
                }
                Err(_) => {
                    let mut child = child_lock.lock().await;
                    match child.kill() {
                        Ok(_) => {}
                        Err(err) => {
                            warn!("Failed to kill pty child: {}", err)
                        }
                    }
                    drop(child);
                }
            }
        });

        Ok(pty)
    }
}

async fn op_register_pty(
    _args: Vec<String>,
    channel: i32,
    sessions: Option<Vec<i32>>,
    _env: Option<HashMap<String, String>>,
) -> Result<u32> {
    let mut env = crate::CHILD_PROCS_ENV_BASE.read().await.clone();

    if let Some(env_vars) = _env {
        env.extend(env_vars);
    }

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

    // Spawn a shell into the pty
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

    // Read and parse output from the pty with reader
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let pty_id = child.process_id().expect("Missing process id????");

    let child_lock = Arc::new(Mutex::new(child));

    let child_lock_reaper = child_lock.clone();
    tokio::task::spawn(async move {
        if let Err(err) = tokio::task::spawn_blocking(move || {
            std::io::copy(&mut reader, &mut PtyWriter { channel, pty_id })
        })
        .await
        {
            error!("Error occurred copying from pty to channels: {}", err);
        };

        let _read = crate::CHANNEL_MESSAGES.read().await;
        if !_read.contains_key(&channel) {
            return Err(AnyError::new(Error::new(
                std::io::ErrorKind::NotFound,
                "Owning channel",
            )));
        }

        let exit_code;

        if let Some(code) = child_lock_reaper.lock().await.try_wait()? {
            exit_code = code.exit_code() as i32;
        } else {
            exit_code = 0;
        }

        let queue = _read.get(&channel).unwrap().clone();
        drop(_read);
        queue.push(ChannelMessage::ProcessDead(pty_id, exit_code));
        Ok(())
    });

    let queue = Arc::new(deadqueue::unlimited::Queue::new());

    if let Some(session_map) = sessions {
        PTY_SESSION_MAP.write().await.insert(pty_id, session_map);
    } else {
        PTY_SESSION_MAP.write().await.insert(pty_id, vec![]);
    }

    let mut pty_channel_writer = crate::PROCCESS_CHANNEL_TO_ID.write().await;
    if pty_channel_writer.contains_key(&channel) {
        drop(pty_channel_writer);
        return Err(AnyError::new(Error::new(
            ErrorKind::AlreadyExists,
            "Channel already has a PTY/CMD",
        )));
    } else {
        pty_channel_writer.insert(channel, pty_id);
    }

    drop(pty_channel_writer);

    crate::PROCCESS_WRITE_MESSAGES
        .write()
        .await
        .insert(pty_id, queue.clone());

    let (task, handle) = abortable(async move {
        loop {
            let task = queue.pop().await;
            if let Err(err) = writer.write(task.as_bytes()) {
                return err;
            }
        }
    });

    PTY_CANCELLATION_MAP.write().await.insert(pty_id, handle);

    tokio::spawn(async move {
        match task.await {
            Ok(err) => {
                error!("Error occurred while passing writes to pty: {}", err)
            }
            Err(_) => {
                let mut child = child_lock.lock().await;
                match child.kill() {
                    Ok(_) => {}
                    Err(err) => {
                        warn!("Failed to kill pty child: {}", err)
                    }
                }
                drop(child);
            }
        }
    });

    Ok(pty_id)
}

// async fn op_pty_write_msg(id: u32, msg: String) -> Result<()> {
//     match crate::PROCCESS_WRITE_MESSAGES.read().await.get(&id) {
//         Some(queue) => {
//             queue.push(msg);

//             Ok(())
//         }
//         None => Err(format_err!("Couldn't find pty {} to write to", id)),
//     }
// }

// async fn op_destroy_pty(id: u32, channel_id: i32) -> Result<()> {
//     if let Some(cancel) = PTY_CANCELLATION_MAP.read().await.get(&id) {
//         cancel.abort();
//     } else {
//         return Err(format_err!("Couldn't find pty {} to write to", id));
//     }

//     PTY_CANCELLATION_MAP.write().await.remove(&id);
//     PTY_SESSION_MAP.write().await.remove(&id);
//     crate::PROCCESS_WRITE_MESSAGES.write().await.remove(&id);
//     crate::PROCCESS_CHANNEL_TO_ID
//         .write()
//         .await
//         .remove(&channel_id);

//     Ok(())
// }

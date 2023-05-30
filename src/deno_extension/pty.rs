use futures_util::{future::abortable, stream::AbortHandle};
use log::error;
use portable_pty::PtySize;
use std::{
    collections::{HashMap, VecDeque},
    io::{Error, ErrorKind, Write},
    sync::Arc,
};

use crate::channels::IPCMessage;

use tokio::sync::Mutex;

use deno_core::{error::AnyError, op, OpDecl};

use lazy_static::lazy_static;

use crate::JsMessage;

lazy_static! {
    static ref MAX_SESSION: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

lazy_static! {
    static ref PTY_CANCELLATION_MAP: c_map::HashMap<u32, AbortHandle> = c_map::HashMap::new();
    static ref PTY_SESSION_MAP: c_map::HashMap<u32, Vec<i32>> = c_map::HashMap::new();
}

struct PtyWriter {
    channel: i32,
    pty_id: u32,
}

impl Write for PtyWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut cmd = crate::goval::Command::default();
        let output: String;
        match String::from_utf8(buf.to_vec()) {
            Ok(str) => output = str,
            Err(err) => {
                error!("Invalid utf-8 output in pty handler");

                return Err(Error::new(ErrorKind::Other, err.utf8_error()));
            }
        }

        cmd.body = Some(crate::goval::command::Body::Output(output));
        cmd.channel = self.channel;

        let sessions;
        let _key = PTY_SESSION_MAP.read(&self.pty_id);
        if let Some(session_map) = _key.get() {
            sessions = session_map;
        } else {
            return Err(std::io::Error::new(
                ErrorKind::ConnectionAborted,
                "Pty is gone",
            ));
        }

        for session in sessions.iter() {
            if let Some(sender) = crate::SESSION_MAP.read(session).get() {
                let mut to_send = cmd.clone();
                to_send.session = *session;

                match sender.send(IPCMessage::from_cmd(to_send, *session)) {
                    Ok(_) => {}
                    Err(err) => {
                        return Err(Error::new(ErrorKind::Other, err));
                    }
                }
            } else {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "Missing session in pty writer",
                ));
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[op]
async fn op_register_pty(
    _args: Vec<String>,
    channel: i32,
    sessions: Option<Vec<i32>>,
    env: Option<HashMap<String, String>>,
) -> Result<u32, AnyError> {
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

    if let Some(env_vars) = env {
        for (key, val) in env_vars.into_iter() {
            cmd.env(key, val)
        }
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
        queue.push(JsMessage::ProcessDead(pty_id, exit_code));
        Ok(())
    });

    let queue = Arc::new(deadqueue::unlimited::Queue::new());

    if let Some(session_map) = sessions {
        PTY_SESSION_MAP.write(pty_id).insert(session_map);
    } else {
        PTY_SESSION_MAP.write(pty_id).insert(vec![]);
    }

    let pty_channel_writer = crate::PROCCESS_CHANNEL_TO_ID.write(channel);
    if pty_channel_writer.contains_key() {
        drop(pty_channel_writer);
        return Err(AnyError::new(Error::new(
            ErrorKind::AlreadyExists,
            "Channel already has a PTY/CMD",
        )));
    } else {
        pty_channel_writer.insert(pty_id);
    }

    crate::PROCCESS_WRITE_MESSAGES
        .write(pty_id)
        .insert(queue.clone());

    let (task, handle) = abortable(async move {
        loop {
            let task = queue.pop().await;
            if let Err(err) = writer.write(task.as_bytes()) {
                return err;
            }
        }
    });

    PTY_CANCELLATION_MAP.write(pty_id).insert(handle);

    tokio::spawn(async move {
        match task.await {
            Ok(err) => {
                error!("Error occurred while passing writes to pty: {}", err)
            }
            Err(_) => {
                let mut child = child_lock.lock().await;
                child.kill().expect("Failed to kill pty child");
                drop(child);
            }
        }
    });

    Ok(pty_id)
}

#[op]
async fn op_pty_add_session(id: u32, session: i32) -> Result<(), AnyError> {
    PTY_SESSION_MAP
        .write(id)
        .entry()
        .and_modify(|sessions| sessions.push(session));
    Ok(())
}

#[op]
async fn op_pty_remove_session(id: u32, session: i32) -> Result<(), AnyError> {
    PTY_SESSION_MAP.write(id).entry().and_modify(|sessions| {
        if let Some(pos) = sessions.iter().position(|x| *x == session) {
            sessions.swap_remove(pos);
        }
    });
    Ok(())
}

#[op]
async fn op_pty_write_msg(id: u32, msg: String) -> Result<(), AnyError> {
    match crate::PROCCESS_WRITE_MESSAGES.read(&id).get() {
        Some(queue) => {
            queue.push(msg);

            Ok(())
        }
        None => Err(AnyError::new(Error::new(
            ErrorKind::NotFound,
            format!("Couldn't find pty {} to write to", id),
        ))),
    }
}

#[op]
async fn op_destroy_pty(id: u32, channel_id: i32) -> Result<(), AnyError> {
    if let Some(cancel) = PTY_CANCELLATION_MAP.read(&id).get() {
        cancel.abort();
    } else {
        return Err(AnyError::new(Error::new(
            ErrorKind::NotFound,
            format!("Couldn't find pty {} to destroy", id),
        )));
    }

    PTY_CANCELLATION_MAP.write(id).remove();
    PTY_SESSION_MAP.write(id).remove();
    crate::PROCCESS_WRITE_MESSAGES.write(id).remove();
    crate::PROCCESS_CHANNEL_TO_ID
        .write(channel_id.clone())
        .remove();

    Ok(())
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_destroy_pty::decl(),
        op_register_pty::decl(),
        op_pty_write_msg::decl(),
        op_pty_add_session::decl(),
        op_pty_remove_session::decl(),
    ]
}

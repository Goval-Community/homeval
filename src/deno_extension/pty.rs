use futures_util::{future::abortable, stream::AbortHandle};
use log::error;
use portable_pty::PtySize;
use std::{
    collections::VecDeque,
    io::{Error, ErrorKind, Write},
    sync::Arc,
};

use crate::channels::IPCMessage;

use tokio::sync::Mutex;

use deno_core::{error::AnyError, op, OpDecl};

use lazy_static::lazy_static;

lazy_static! {
    static ref MAX_SESSION: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

use c_map::HashMap;
lazy_static! {
    static ref PTY_CANCELATION_MAP: HashMap<u32, AbortHandle> = HashMap::new();
    static ref PTY_SESSION_MAP: HashMap<u32, Vec<i32>> = HashMap::new();
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

        for session in PTY_SESSION_MAP.read(&self.pty_id).get().unwrap().iter() {
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
async fn op_register_pty(_args: Vec<String>, channel: i32) -> Result<u32, AnyError> {
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

    let child = pair.slave.spawn_command(cmd)?;

    // Read and parse output from the pty with reader
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let pty_id = child.process_id().expect("Missing process id????");

    tokio::task::spawn(async move {
        tokio::task::spawn_blocking(move || {
            std::io::copy(&mut reader, &mut PtyWriter { channel, pty_id })
        });
    });

    let queue = Arc::new(deadqueue::unlimited::Queue::new());

    PTY_SESSION_MAP.write(pty_id).insert(vec![]);
    let mut pty_channel_map_writer = crate::PTY_CHANNEL_TO_ID.write(channel);
    if pty_channel_map_writer.contains_key() {
        pty_channel_map_writer
            .entry()
            .and_modify(|pty_channel_map| {
                pty_channel_map.push(pty_id);
            });
    } else {
        pty_channel_map_writer.insert(vec![pty_id]);
    }
    crate::PTY_WRITE_MESSAGES
        .write(pty_id)
        .insert(queue.clone());

    let (task, handle) = abortable(async move {
        loop {
            let task = queue.pop().await;
            writer
                .write(task.as_bytes())
                .expect("Error writing bytes to pty :/");
        }
    });

    PTY_CANCELATION_MAP.write(pty_id).insert(handle);

    tokio::spawn(task);

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
    match crate::PTY_WRITE_MESSAGES.read(&id).get() {
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
async fn op_destroy_pty(id: u32) -> Result<(), AnyError> {
    match PTY_CANCELATION_MAP.read(&id).get() {
        Some(cancel) => {
            cancel.abort();

            Ok(())
        }
        None => Err(AnyError::new(Error::new(
            ErrorKind::NotFound,
            format!("Couldn't find pty {} to destroy", id),
        ))),
    }
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

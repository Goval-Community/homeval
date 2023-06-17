use futures_util::{future::abortable, stream::AbortHandle, Future};
use log::error;
use std::{
    collections::{HashMap, VecDeque},
    io::{Error, ErrorKind},
    pin::Pin,
    process::Stdio,
    sync::Arc,
    task::{Context, Poll},
};

use crate::channels::IPCMessage;

use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    process::Command,
    sync::{Mutex, RwLock},
};

use deno_core::{error::AnyError, op, OpDecl};

use lazy_static::lazy_static;

use crate::JsMessage;

lazy_static! {
    static ref MAX_SESSION: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

lazy_static! {
    static ref CMD_CANCELLATION_MAP: RwLock<HashMap<u32, AbortHandle>> =
        RwLock::new(HashMap::new());
    static ref CMD_SESSION_MAP: RwLock<HashMap<u32, Vec<i32>>> = RwLock::new(HashMap::new());
}

struct CmdWriter {
    channel: i32,
    cmd_id: u32,
}
async fn remove_refs(id: u32, channel_id: i32) {
    CMD_CANCELLATION_MAP.write().await.remove(&id);
    CMD_SESSION_MAP.write().await.remove(&id);
    crate::PROCCESS_WRITE_MESSAGES.write().await.remove(&id);
    crate::PROCCESS_CHANNEL_TO_ID
        .write()
        .await
        .remove(&channel_id);
}

async fn write_to_cmd(buf: &[u8], channel: i32, cmd_id: u32) -> Result<usize, Error> {
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

    cmd.channel = channel;

    let sessions;
    let _key = CMD_SESSION_MAP.read().await;
    if let Some(session_map) = _key.get(&cmd_id) {
        sessions = session_map;
    } else {
        return Err(std::io::Error::new(
            ErrorKind::ConnectionAborted,
            "Cmd is gone",
        ));
    }

    for session in sessions.iter() {
        if let Some(sender) = crate::SESSION_MAP.read().await.get(&session) {
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
                "Missing session in cmd writer",
            ));
        }
    }

    Ok(buf.len())
}

impl AsyncWrite for CmdWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        let mut future: Pin<Box<dyn Future<Output = Result<usize, Error>>>> =
            Box::pin(write_to_cmd(buf, self.channel, self.cmd_id));
        future.as_mut().poll(cx)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
}

#[op]
async fn op_register_cmd(
    _args: Vec<String>,
    channel: i32,
    sessions: Option<Vec<i32>>,
    _env: Option<HashMap<String, String>>,
) -> Result<u32, AnyError> {
    let mut env = crate::CHILD_PROCS_ENV_BASE.read().await.clone();

    if let Some(env_vars) = _env {
        env.extend(env_vars);
    }

    let args = &mut VecDeque::from(_args.to_vec());
    let mut cmd = Command::new(VecDeque::pop_front(args).expect("Missing command"));
    for arg in args {
        cmd.arg(arg);
    }
    cmd.current_dir(std::env::current_dir()?);

    cmd.envs(env);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::piped());

    let mut child = cmd.spawn()?;

    // let output_channel = oneshot::channel::<Output>();

    // tokio::spawn(async {});

    // Read and parse output from the cmd with reader
    let stdout_opt = child.stdout.take();
    let stderr_opt = child.stderr.take();
    let stdin_opt = child.stdin.take();

    let cmd_id = child.id().expect("Missing process id????");

    let queue = Arc::new(deadqueue::unlimited::Queue::new());

    if let Some(session_map) = sessions {
        CMD_SESSION_MAP.write().await.insert(cmd_id, session_map);
    } else {
        CMD_SESSION_MAP.write().await.insert(cmd_id, vec![]);
    }

    let mut cmd_channel_writer = crate::PROCCESS_CHANNEL_TO_ID.write().await;
    if cmd_channel_writer.contains_key(&channel) {
        drop(cmd_channel_writer);
        return Err(AnyError::new(Error::new(
            ErrorKind::AlreadyExists,
            "Channel already has a PTY/CMDs",
        )));
    } else {
        cmd_channel_writer.insert(channel, cmd_id);
    }

    drop(cmd_channel_writer);

    crate::PROCCESS_WRITE_MESSAGES
        .write()
        .await
        .insert(cmd_id, queue.clone());

    let child_lock = Arc::new(Mutex::new(child));

    let (task, handle) = abortable(tokio::spawn(async move {
        tokio::select! {
                res = async move {
                    if let Some(mut stdin) = stdin_opt {
                        loop {
                            let task = queue.pop().await;
                            if let Err(err) = stdin.write(task.as_bytes()).await {
                                return err;
                            }
                        }
                    } else {
                        Error::new(ErrorKind::BrokenPipe, "stdin missing")
                    }
                } => {
                    error!("Error occurred while passing writes to cmd: {}", res);
                },
                _ = async move {
                    let stderr_cpy;
                    let stdout_cpy;

                    if let Some(mut stdout) = stdout_opt {
                    let mut pty_writer_out = CmdWriter { channel, cmd_id };
                    stdout_cpy = Some(async move {
                            tokio::io::copy(&mut stdout, &mut pty_writer_out).await
                    });
                    } else {
                        stdout_cpy = None;
                    }

                    if let Some(mut stderr) = stderr_opt {
                    let mut pty_writer_err = CmdWriter { channel, cmd_id };
                    stderr_cpy = Some(async move {
                            tokio::io::copy(&mut stderr, &mut pty_writer_err).await
                        })
                    } else {
                        stderr_cpy = None;
                    }

                    let final_res;

                    if stderr_cpy.is_some() && stdout_cpy.is_some() {
                        tokio::select! {
                            res = stderr_cpy.unwrap() => {
                                final_res = res
                            }
                            res = stdout_cpy.unwrap() => {
                                final_res = res
                            }
                        }
                    } else if let Some(task) = stderr_cpy {
                        final_res = task.await;
                    } else if let Some(task) = stdout_cpy {
                        final_res = task.await;
                    }
                    // else if child_lock.lock().await.out {

                    // }
                    else {
                        final_res = Err(Error::new(ErrorKind::NotFound, "Both stdout and stderr missing"))
                    }

                    if let Err(err) = final_res {
                        error!("Error occurred copying from cmd to channels: {}", err);
                    };
                } => {},
        }
    }));

    CMD_CANCELLATION_MAP.write().await.insert(cmd_id, handle);

    // tokio::spawn();
    tokio::spawn(async move {
        let res = task.await;

        remove_refs(cmd_id, channel).await;

        let _read = crate::CHANNEL_MESSAGES.read().await;
        if !_read.contains_key(&channel) {
            return Err(AnyError::new(Error::new(
                std::io::ErrorKind::NotFound,
                "Cmd missing owning channel",
            )));
        }

        let mut child = child_lock.lock().await;

        let exit_code;

        if let Some(code) = child.try_wait()? {
            exit_code = code.code().unwrap_or(1);
        } else {
            exit_code = 0;
        }

        let queue = _read.get(&channel).unwrap().clone();
        drop(_read);
        queue.push(JsMessage::ProcessDead(cmd_id, exit_code));

        match res {
            Ok(res) => {
                if let Err(err) = res {
                    error!("Join Error encountered: {}", err);
                }
            }
            Err(_) => {
                child.kill().await.expect("Failed to kill cmd child");
            }
        }

        Ok(())
    });

    Ok(cmd_id)
}

#[op]
async fn op_cmd_add_session(id: u32, session: i32) -> Result<(), AnyError> {
    CMD_SESSION_MAP
        .write()
        .await
        .entry(id)
        .and_modify(|sessions| sessions.push(session));
    Ok(())
}

#[op]
async fn op_cmd_remove_session(id: u32, session: i32) -> Result<(), AnyError> {
    CMD_SESSION_MAP
        .write()
        .await
        .entry(id)
        .and_modify(|sessions| {
            if let Some(pos) = sessions.iter().position(|x| *x == session) {
                sessions.swap_remove(pos);
            }
        });
    Ok(())
}

#[op]
async fn op_cmd_write_msg(id: u32, msg: String) -> Result<(), AnyError> {
    match crate::PROCCESS_WRITE_MESSAGES.read().await.get(&id) {
        Some(queue) => {
            queue.push(msg);

            Ok(())
        }
        None => Err(AnyError::new(Error::new(
            ErrorKind::NotFound,
            format!("Couldn't find cmd {} to write to", id),
        ))),
    }
}

#[op]
async fn op_destroy_cmd(id: u32, channel_id: i32) -> Result<(), AnyError> {
    if let Some(cancel) = CMD_CANCELLATION_MAP.read().await.get(&id) {
        cancel.abort();
    } else {
        return Err(AnyError::new(Error::new(
            ErrorKind::NotFound,
            format!("Couldn't find cmd {} to destroy", id),
        )));
    }

    remove_refs(id, channel_id).await;

    Ok(())
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_destroy_cmd::decl(),
        op_register_cmd::decl(),
        op_cmd_write_msg::decl(),
        op_cmd_add_session::decl(),
        op_cmd_remove_session::decl(),
    ]
}

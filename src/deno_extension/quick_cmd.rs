use homeval::goval;
use log::warn;
use std::{
    collections::{HashMap, VecDeque},
    io::{Error, Read},
    process::Stdio,
};

use crate::channels::IPCMessage;

use tokio::process::Command;

use deno_core::{error::AnyError, op, OpDecl};

use crate::JsMessage;

#[op]
async fn op_run_cmd(
    _args: Vec<String>,
    channel: i32,
    sessions: Vec<i32>,
    env: Option<HashMap<String, String>>,
) -> Result<i32, AnyError> {
    let args = &mut VecDeque::from(_args.to_vec());
    let mut cmd = Command::new(VecDeque::pop_front(args).expect("Missing command"));
    for arg in args {
        cmd.arg(arg);
    }
    cmd.current_dir(std::env::current_dir()?);

    if let Some(env_vars) = env {
        cmd.envs(env_vars);
    }

    let (mut reader, writer) = os_pipe::pipe()?;
    let writer_clone = writer.try_clone()?;
    cmd.stdout(writer);
    cmd.stderr(writer_clone);
    cmd.stdin(Stdio::piped());

    let mut child = cmd.spawn()?;

    drop(cmd);

    let output = tokio::task::spawn_blocking(move || -> Result<String, Error> {
        let mut output = String::new();
        reader.read_to_string(&mut output)?;
        Ok(output)
    })
    .await??;

    let status = child.wait().await?;

    let mut output_cmd = goval::Command::default();

    output_cmd.body = Some(crate::goval::command::Body::Output(output));
    output_cmd.channel = channel;

    for session in sessions.iter() {
        if let Some(sender) = crate::SESSION_MAP.read(session).get() {
            let mut to_send = output_cmd.clone();
            to_send.session = *session;

            sender.send(IPCMessage::from_cmd(to_send, *session))?
        } else {
            warn!(
                "Session {} missing when sending cmd output for channel {}",
                session, channel
            )
        }
    }

    let _read = crate::CHANNEL_MESSAGES.read().await;
    if !_read.contains_key(&channel) {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::NotFound,
            "Cmd missing owning channel",
        )));
    }

    let queue = _read.get(&channel).unwrap().clone();
    drop(_read);

    let exit_code = status.code().unwrap_or(0);
    queue.push(JsMessage::CmdDead(exit_code));

    Ok(exit_code)
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![op_run_cmd::decl()]
}

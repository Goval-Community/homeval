use homeval::goval;
use log::{error, warn};
use std::{
    collections::{HashMap, VecDeque},
    io::Error,
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

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let child = cmd.spawn()?;

    let output = child.wait_with_output().await?;
    // TODO: handle actual status
    let mut status = 0;

    drop(cmd);

    // TODO: handle actual stderr
    if output.stderr.len() > 0 {
        status = 1;
        error!("QUICK CMD STDERR: {}", String::from_utf8(output.stderr)?);
    }

    let mut output_cmd = goval::Command::default();

    output_cmd.body = Some(crate::goval::command::Body::Output(String::from_utf8(
        output.stdout,
    )?));
    output_cmd.channel = channel;

    for session in sessions.iter() {
        if let Some(sender) = crate::SESSION_MAP.read().await.get(session) {
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

    let exit_code = status;
    queue.push(JsMessage::CmdDead(exit_code));

    Ok(exit_code)
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![op_run_cmd::decl()]
}

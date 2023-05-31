use deno_core::{error::AnyError, op, OpDecl};
use log::error;
use serde::{Deserialize, Serialize};
use std::io::Error;

use crate::{channels::IPCMessage, parse_paseto::ClientInfo};

#[op]
async fn op_send_msg(msg: IPCMessage) -> Result<(), AnyError> {
    if let Some(sender) = crate::SESSION_MAP.read(&msg.session.clone()).get() {
        sender.send(msg)?;
    } else {
        error!("Missing session outbound message queue in op_send_msg")
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsMessage {
    #[serde(rename = "ipc")]
    IPC(IPCMessage),
    Attach(i32),
    Detach(i32),
    Close(i32),
    ProcessDead(u32, i32),
    CmdDead(i32),
}

#[op]
async fn op_recv_info(channel: i32) -> Result<JsMessage, AnyError> {
    let _read = crate::CHANNEL_MESSAGES.read().await;
    if !_read.contains_key(&channel) {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        )));
    }
    let queue = _read.get(&channel).unwrap().clone();
    drop(_read);

    let res = queue.pop().await;
    Ok(res)
}

#[op]
fn op_user_info(session: i32) -> Result<ClientInfo, AnyError> {
    let _read = crate::SESSION_CLIENT_INFO.read(&session);
    if !_read.contains_key() {
        return Ok(ClientInfo::default());
    }

    match _read.get() {
        Some(info) => Ok(info.clone()),
        None => Ok(ClientInfo::default()),
    }
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_recv_info::decl(),
        op_send_msg::decl(),
        op_user_info::decl(),
    ]
}

use deno_core::{error::AnyError, op, OpDecl};
use log::error;
use serde::{Deserialize, Serialize};
use std::io::Error;

use crate::channels::IPCMessage;

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
}

#[op]
async fn op_recv_info(channel: i32) -> Result<JsMessage, AnyError> {
    // let queues_clone = CHANNEL_MESSAGES.clone();
    // let internal = 0 as i32;
    // info!("Checking for channel: {} in queue list", internal);
    let _read = crate::CHANNEL_MESSAGES.read(&channel);
    if !_read.contains_key() {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        )));
    }
    let queue = _read.get().unwrap();

    let res = queue.pop().await;
    Ok(res)
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![op_recv_info::decl(), op_send_msg::decl()]
}

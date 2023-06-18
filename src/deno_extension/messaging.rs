use deno_core::{error::AnyError, op, OpDecl};
use log::{as_debug, error, trace};
use serde::{Deserialize, Serialize};
use std::io::Error;

use crate::{channels::IPCMessage, parse_paseto::ClientInfo};

#[op]
async fn op_send_msg(msg: IPCMessage) -> Result<(), AnyError> {
    if let Some(sender) = crate::SESSION_MAP.read().await.get(&msg.session.clone()) {
        sender.send(msg)?;
    } else {
        error!("Missing session outbound message queue in op_send_msg")
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum JsMessage {
    #[serde(rename = "ipc")]
    IPC(IPCMessage),
    Attach(i32),
    Detach(i32),
    Close(i32),
    ProcessDead(u32, i32),
    CmdDead(i32),
    Replspace(i32, ReplspaceMessage), // session, message
    Shutdown(bool), // Shutdown the service, value has to be true so that runtime.js can match it in an if check
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ReplspaceMessage {
    GithubTokenReq(String),                 // nonce
    OpenFileReq(String, bool, String),      // file, wait for close, nonce
    OpenMultipleFiles(Vec<String>, String), // files, nonce

    GithubTokenRes(String), // token
    OpenFileRes,
}

#[op]
async fn op_ack_shutdown(channel: i32) -> Result<(), AnyError> {
    trace!(channel = channel; "Channel ack'ed shutdown");
    crate::CHANNEL_MESSAGES.write().await.remove(&channel);
    Ok(())
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
async fn op_replspace_reply(nonce: String, reply: ReplspaceMessage) -> Result<(), AnyError> {
    crate::REPLSPACE_CALLBACKS
        .write()
        .await
        .entry(nonce.clone())
        .and_modify(|entry| {
            let sender = entry.take().unwrap();
            // let sender = Arc::try_unwrap(_sender.clone()).unwrap();
            match sender.send(reply) {
                Ok(_) => {}
                Err(val) => error!(
                    message = as_debug!(val);
                    "Failed to send replspace api reply"
                ),
            };
        });
    Ok(())
}

#[op]
async fn op_user_info(session: i32) -> Result<ClientInfo, AnyError> {
    let _read = crate::SESSION_CLIENT_INFO.read().await;
    if !_read.contains_key(&session) {
        return Ok(ClientInfo::default());
    }

    match _read.get(&session) {
        Some(info) => Ok(info.clone()),
        None => Ok(ClientInfo::default()),
    }
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_ack_shutdown::decl(),
        op_recv_info::decl(),
        op_send_msg::decl(),
        op_user_info::decl(),
        op_replspace_reply::decl(),
    ]
}

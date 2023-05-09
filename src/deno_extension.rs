use deno_core::{op, Extension};
use serde::{Deserialize, Serialize};
use tokio::fs;

use tokio_stream::{wrappers::ReadDirStream, StreamExt};

use deno_core::error::AnyError;

use crate::channels::IPCMessage;
use log::{log, warn};
use std::io::Error;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub service: String,
    pub name: Option<String>,
    pub id: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub path: String,
    pub r#directory: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsoleLogLevels {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl ConsoleLogLevels {
    pub fn to_log(&self) -> log::Level {
        match self {
            ConsoleLogLevels::Trace => log::Level::Trace,
            ConsoleLogLevels::Debug => log::Level::Debug,
            ConsoleLogLevels::Info => log::Level::Info,
            ConsoleLogLevels::Warn => log::Level::Warn,
            ConsoleLogLevels::Error => log::Level::Error,
        }
    }
}

#[op]
fn op_console_log(
    level: ConsoleLogLevels,
    service: Service,
    message: String,
) -> Result<(), AnyError> {
    let mut name = "".to_string();
    match service.name {
        None => {}
        Some(_name) => {
            name = format!(":{}", _name);
        }
    }
    let target = &format!("goval_impl/v8: {}{}", service.service, name);

    log!(target: target, level.to_log(), "{}", message);
    Ok(())
}

#[op]
async fn op_send_msg(msg: IPCMessage) -> Result<(), AnyError> {
    crate::SESSION_MAP
        .read(&msg.session.clone())
        .get()
        .unwrap()
        .send(msg)
        .unwrap();
    Ok(())
}

#[op]
async fn op_recv_info(channel: i32) -> Result<IPCMessage, AnyError> {
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

#[op]
async fn op_list_dir(path: String) -> Result<Vec<File>, AnyError> {
    let mut dir = ReadDirStream::new(fs::read_dir(path.clone()).await?);
    let mut ret = Vec::<File>::new();
    let parent = std::path::Path::new(&path);
    while let Some(fs_path) = dir.next().await {
        let file = fs_path?;
        let path = file
            .path()
            .strip_prefix(parent)?
            .to_str()
            .unwrap()
            .to_string();
        let file_type = file.file_type().await?;
        let is_dir = file_type.is_dir();
        if !is_dir && !file_type.is_file() {
            warn!("File: {}, is not a directory or a file", path);
        }
        ret.push(File {
            path,
            directory: is_dir,
        });
    }
    Ok(ret)
}

pub fn make_extension() -> Extension {
    Extension::builder()
        .ops(vec![
            op_recv_info::decl(),
            op_send_msg::decl(),
            op_console_log::decl(),
            op_list_dir::decl(),
        ])
        .build()
}

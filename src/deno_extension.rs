use deno_core::{op, Extension};
use serde::{Deserialize, Serialize};
use tokio::{fs, io::AsyncWriteExt};

use tokio_stream::{wrappers::ReadDirStream, StreamExt};

use deno_core::error::AnyError;

use crate::channels::IPCMessage;
use log::{error, info, log};
use std::io::Error;

#[op]
async fn op_send_msg(msg: IPCMessage) -> Result<(), AnyError> {
    if let Some(sender) = crate::SESSION_MAP.read(&msg.session.clone()).get() {
        sender.send(msg)?;
    } else {
        error!("Missing session outbound message queue in op_send_msg")
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JsMessage {
    #[serde(rename = "ipc")]
    IPC(IPCMessage),
    Attach(i32),
    Detach(i32),
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub service: String,
    pub name: Option<String>,
    pub id: i32,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub path: String,
    pub r#type: FileType,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

impl FileType {
    pub fn from_file_type(file_type: std::fs::FileType) -> Result<Self, AnyError> {
        let ret: FileType;

        if file_type.is_dir() {
            ret = FileType::Directory
        } else if file_type.is_file() {
            ret = FileType::File
        } else if file_type.is_symlink() {
            ret = FileType::Symlink
        } else {
            return Err(AnyError::new(Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid file type",
            )));
        }

        Ok(ret)
    }
}

#[op]
async fn op_list_dir(path: String) -> Result<Vec<File>, AnyError> {
    let mut dir = ReadDirStream::new(fs::read_dir(path.clone()).await?);
    let mut ret = Vec::<File>::new();
    let parent = std::path::Path::new(&path);
    while let Some(fs_path) = dir.next().await {
        let file = fs_path?;
        if let Some(str_path) = file.path().strip_prefix(parent)?.to_str() {
            let file_type = FileType::from_file_type(file.file_type().await?)?;

            ret.push(File {
                path: str_path.to_string(),
                r#type: file_type,
            });
        } else {
            error!("Got none from Path#to_str in op_list_dir")
        }
    }
    Ok(ret)
}

#[op]
async fn op_write_file(path: String, contents: Vec<u8>) -> Result<(), AnyError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(path)
        .await?;
    file.set_len(0).await?;
    file.write(&contents).await?;
    Ok(())
}

#[op]
async fn op_read_file(path: String) -> Result<Vec<u8>, AnyError> {
    Ok(fs::read(path).await?)
}

#[op]
async fn op_remove_file(path: String) -> Result<(), AnyError> {
    let stat = fs::metadata(path.clone()).await?;
    if stat.is_dir() {
        Ok(fs::remove_dir_all(path).await?)
    } else {
        Ok(fs::remove_file(path).await?)
    }
}

#[op]
async fn op_move_file(old_path: String, new_path: String) -> Result<(), AnyError> {
    Ok(fs::rename(old_path, new_path).await?)
}

pub fn make_extension() -> Extension {
    Extension::builder()
        .ops(vec![
            // Send / recv msgs
            op_recv_info::decl(),
            op_send_msg::decl(),
            // Console
            op_console_log::decl(),
            // FS
            op_list_dir::decl(),
            op_write_file::decl(),
            op_read_file::decl(),
            op_remove_file::decl(),
            op_move_file::decl(),
        ])
        .build()
}

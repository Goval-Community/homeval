use std::{io::Error, os::unix::prelude::MetadataExt};

use deno_core::{error::AnyError, op, OpDecl};
use log::error;
use serde::Serialize;
use tokio::{fs, io::AsyncWriteExt};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub exists: bool,
    #[serde(rename = "type")]
    pub file_type: FileType,
    pub size: u64,
    pub file_mode: String,
    pub mod_time: i64,
}

async fn inner_stat(path: String) -> Result<Option<FileStat>, AnyError> {
    match fs::metadata(path).await {
        Err(_) => Ok(None),
        Ok(stat_info) => Ok(Some(FileStat {
            exists: true,
            file_type: FileType::from_file_type(stat_info.file_type())?,
            size: stat_info.size(),
            file_mode: "".to_string(),
            mod_time: stat_info.mtime(),
        })),
    }
}

// TODO: function to stat multiple files at once

#[op]
async fn op_stat_file(path: String) -> Result<FileStat, AnyError> {
    match inner_stat(path.clone()).await? {
        None => Err(Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", path),
        )
        .into()),
        Some(stat) => Ok(stat),
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

#[op]
async fn op_read_file_string(path: String) -> Result<String, AnyError> {
    Ok(String::from_utf8(fs::read(path).await?)?)
}

#[op]
async fn op_write_file_string(path: String, contents: String) -> Result<(), AnyError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(path)
        .await?;
    file.set_len(0).await?;
    file.write(&contents.as_bytes()).await?;
    Ok(())
}

#[op]
fn op_get_working_dir() -> Result<String, AnyError> {
    // TODO: deal with possible panic from unwrap
    Ok(std::env::current_dir()?.to_str().unwrap().to_string())
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_stat_file::decl(),
        op_list_dir::decl(),
        op_write_file::decl(),
        op_read_file::decl(),
        op_remove_file::decl(),
        op_move_file::decl(),
        op_read_file_string::decl(),
        op_write_file_string::decl(),
        op_get_working_dir::decl(),
    ]
}

pub struct GCSFiles {}

use super::traits;
use anyhow::{format_err, Result};
use async_trait::async_trait;
use tokio::{fs, io::AsyncWriteExt};
use tracing::{debug, warn};

#[async_trait]
impl traits::Service for GCSFiles {
    async fn message(
        &mut self,
        _info: &super::types::ChannelInfo,
        message: goval::Command,
        _session: i32,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };

        match body {
            goval::command::Body::Readdir(dir) => {
                let parent = std::path::Path::new(&dir.path);

                let mut res: Vec<goval::File> = vec![];
                let mut iter = fs::read_dir(&parent).await?;

                while let Some(file) = iter.next_entry().await? {
                    let mut entry = goval::File::default();
                    if let Some(str_path) = file.path().strip_prefix(parent)?.to_str() {
                        entry.path = str_path.to_string();

                        let ftype = file.metadata().await?;
                        if ftype.is_dir() {
                            entry.r#type = goval::file::Type::Directory.into();
                        } else {
                            entry.r#type = goval::file::Type::Regular.into();
                        }

                        res.push(entry);
                    } else {
                        return Err(format_err!("Got none from Path#to_str in gcsfiles#readdir"));
                    }
                }

                let mut ret = goval::Command::default();
                let _inner = goval::Files { files: res };
                ret.body = Some(goval::command::Body::Files(_inner));
                Ok(Some(ret))
            }
            goval::command::Body::Mkdir(dir) => {
                fs::create_dir_all(dir.path).await?;
                let ret = goval::Command {
                    body: Some(goval::command::Body::Ok(goval::Ok {})),
                    ..Default::default()
                };
                Ok(Some(ret))
            }
            goval::command::Body::Read(file) => {
                debug!(path = file.path, "File path");
                let contents = match file.path.as_str() {
                    // TODO: Read this from in the db
                    ".env" => vec![],
                    ".config/goval/info" => {
                        let val = serde_json::json!({
                            "server": "homeval",
                            "version": env!("CARGO_PKG_VERSION").to_string(),
                            "license": "AGPL",
                            "authors": vec!["PotentialStyx <62217716+PotentialStyx@users.noreply.github.com>"],
                            "repository": "https://github.com/goval-community/homeval",
                            "description": "", // TODO: do dis
                            "uptime": 0, // TODO: impl fo realz
                            "services": super::IMPLEMENTED_SERVICES
                        });

                        val.to_string().as_bytes().to_vec()
                    }
                    _ => match fs::read(&file.path).await {
                        Err(err) => {
                            warn!(error = %err, "Error reading file in gcsfiles");
                            let ret = goval::Command {
                                body: Some(goval::command::Body::Error(format!(
                                    "{}: no such file or directory",
                                    file.path
                                ))),
                                ..Default::default()
                            };

                            return Ok(Some(ret));
                        }
                        Ok(contents) => contents,
                    },
                };

                let mut ret = goval::Command::default();
                let mut _inner = goval::File {
                    content: contents,
                    path: file.path,
                    ..Default::default()
                };
                ret.body = Some(goval::command::Body::File(_inner));
                Ok(Some(ret))
            }
            goval::command::Body::Remove(file) => {
                let stat = fs::metadata(&file.path).await?;
                if stat.is_dir() {
                    fs::remove_dir_all(&file.path).await?
                } else {
                    fs::remove_file(&file.path).await?
                }

                let ret = goval::Command {
                    body: Some(goval::command::Body::Ok(goval::Ok {})),
                    ..Default::default()
                };
                Ok(Some(ret))
            }
            goval::command::Body::Move(move_req) => {
                fs::rename(move_req.old_path, move_req.new_path).await?;
                let ret = goval::Command {
                    body: Some(goval::command::Body::Ok(goval::Ok {})),
                    ..Default::default()
                };
                Ok(Some(ret))
            }
            goval::command::Body::Write(_file) => {
                // TODO: Store this in the db
                if &_file.path == ".env" {
                    let ret = goval::Command {
                        body: Some(goval::command::Body::Ok(goval::Ok {})),
                        ..Default::default()
                    };

                    return Ok(Some(ret));
                }

                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(_file.path)
                    .await?;
                file.set_len(0).await?;
                file.write_all(&_file.content).await?;
                let ret = goval::Command {
                    body: Some(goval::command::Body::Ok(goval::Ok {})),
                    ..Default::default()
                };
                Ok(Some(ret))
            }
            goval::command::Body::Stat(_) => {
                // TODO: impl
                Ok(None)
            }
            _ => {
                warn!(cmd = ?message, "Unknown gcsfiles command");
                Ok(None)
            }
        }
    }
}

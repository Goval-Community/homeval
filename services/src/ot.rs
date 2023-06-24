pub struct OT {
    version: u32,
    contents: ropey::Rope,
    path: String,
    cursors: HashMap<String, goval::OtCursor>,
    history: Vec<goval::OtPacket>,
}
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::ClientInfo;

use super::traits;
use anyhow::{format_err, Result};
use async_trait::async_trait;
use log::{as_debug, warn};
use tokio::fs;
impl OT {
    pub fn new() -> OT {
        OT {
            version: 1,
            contents: "".into(),
            path: "".to_string(),
            cursors: HashMap::new(),
            history: vec![],
        }
    }
}

#[async_trait]
impl traits::Service for OT {
    async fn open(&mut self, _info: &super::types::ChannelInfo) -> Result<()> {
        Ok(())
    }

    async fn message(
        &mut self,
        info: &super::types::ChannelInfo,
        message: goval::Command,
        session: i32,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };

        if &self.path == "" {
            if let goval::command::Body::OtLinkFile(link_file) = body.clone() {
                let path = link_file.file.unwrap().path;
                match fs::metadata(path.clone()).await {
                    Err(_) => {
                        let mut error = goval::Command::default();
                        error.body = Some(goval::command::Body::Error(format!(
                            "{}: no such file or directory",
                            path
                        )));
                        return Ok(Some(error));
                    }
                    Ok(_) => {}
                };

                self.path = path.clone();
                let byte_contents = fs::read(path.clone()).await?;
                let crc32 = crc32fast::hash(byte_contents.as_slice());

                self.contents = String::from_utf8(byte_contents.clone())?.into();

                let timestamp = Some(prost_types::Timestamp {
                    seconds: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                    nanos: 0,
                });

                let hist_item = goval::OtPacket {
                    spooky_version: self.version,
                    version: self.version,
                    op: vec![],
                    crc32,
                    committed: timestamp,
                    author: goval::ot_packet::Author::User.into(),
                    user_id: 23352071,
                    nonce: 0,
                };

                self.history.push(hist_item);

                let mut link_response = goval::Command::default();

                let mut file = goval::File::default();
                file.path = path;
                file.content = byte_contents;

                let _inner = goval::OtLinkFileResponse {
                    version: self.version,
                    linked_file: Some(file),
                };
                link_response.body = Some(goval::command::Body::OtLinkFileResponse(_inner));

                return Ok(Some(link_response));
            } else {
                return Err(format_err!("Command sent before otLinkFile"));
            }
        }

        match body {
            goval::command::Body::Ot(ot) => {
                let mut cursor: usize = 0;

                for op in ot.op.clone() {
                    match op.op_component.unwrap() {
                        goval::ot_op_component::OpComponent::Skip(_skip) => {
                            let skip: usize = _skip.try_into()?;
                            if skip + cursor > self.contents.len_chars() {
                                let mut err = goval::Command::default();
                                err.body = Some(goval::command::Body::Error(
                                    "Invalid skip past bounds".to_string(),
                                ));
                                return Ok(Some(err));
                            }

                            cursor += skip
                        }
                        goval::ot_op_component::OpComponent::Delete(_delete) => {
                            let delete: usize = _delete.try_into()?;
                            if delete + cursor > self.contents.len_chars() {
                                let mut err = goval::Command::default();
                                err.body = Some(goval::command::Body::Error(
                                    "Invalid delete past bounds".to_string(),
                                ));
                                return Ok(Some(err));
                            }

                            self.contents.remove(cursor..(cursor + delete))
                        }
                        goval::ot_op_component::OpComponent::Insert(insert) => {
                            self.contents.insert(cursor, &insert)
                        }
                    }
                }

                let to_write = self.contents.to_string();
                self.version += 1;
                let user_id = 22261053;
                let crc32 = crc32fast::hash(to_write.as_bytes());

                let packet = goval::OtPacket {
                    spooky_version: self.version,
                    version: self.version,
                    op: ot.op,
                    committed: None,
                    crc32,
                    nonce: 0,
                    user_id,
                    author: ot.author,
                };

                self.history.push(packet.clone());

                let mut ot_notif = goval::Command::default();
                ot_notif.body = Some(goval::command::Body::Ot(packet));

                info.send(ot_notif, crate::SendSessions::Everyone).await?;

                fs::write(&self.path, to_write).await?;

                let mut ok = goval::Command::default();
                ok.body = Some(goval::command::Body::Ok(goval::Ok {}));
                Ok(Some(ok))
            }
            goval::command::Body::OtNewCursor(cursor) => {
                self.cursors.insert(cursor.id.clone(), cursor.clone());

                let mut cursor_notif = goval::Command::default();

                cursor_notif.body = Some(goval::command::Body::OtNewCursor(cursor));

                info.send(cursor_notif, crate::SendSessions::EveryoneExcept(session))
                    .await?;
                Ok(None)
            }
            goval::command::Body::OtDeleteCursor(cursor) => {
                self.cursors.remove(&cursor.id);

                let mut cursor_delete_notif = goval::Command::default();

                cursor_delete_notif.body = Some(goval::command::Body::OtDeleteCursor(cursor));

                info.send(
                    cursor_delete_notif,
                    crate::SendSessions::EveryoneExcept(session),
                )
                .await?;

                Ok(None)
            }
            goval::command::Body::Flush(_) => {
                let mut ok = goval::Command::default();
                ok.body = Some(goval::command::Body::Ok(goval::Ok {}));
                Ok(Some(ok))
            }
            _ => {
                warn!(cmd = as_debug!(message); "Unknown ot command");
                Ok(None)
            }
        }
    }

    async fn attach(
        &mut self,
        _info: &super::types::ChannelInfo,
        _client: ClientInfo,
        _session: i32,
    ) -> Result<Option<goval::Command>> {
        if &self.path == "" {
            let mut cmd = goval::Command::default();
            cmd.body = Some(goval::command::Body::Otstatus(goval::OtStatus::default()));
            return Ok(Some(cmd));
        }
        let mut status = goval::Command::default();

        let mut file = goval::File::default();
        file.path = self.path.clone();

        let mut cursors = vec![];

        for cursor in self.cursors.values() {
            cursors.push(cursor.clone())
        }

        let _inner = goval::OtStatus {
            contents: self.contents.to_string(),
            version: self.version,
            linked_file: Some(file),
            cursors: cursors,
        };
        status.body = Some(goval::command::Body::Otstatus(_inner));

        Ok(Some(status))
    }
}

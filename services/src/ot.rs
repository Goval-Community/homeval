pub struct OT {
    crc32: Arc<RwLock<u32>>,
    version: Arc<RwLock<u32>>,
    contents: Arc<RwLock<ropey::Rope>>,
    path: String,
    cursors: HashMap<String, goval::OtCursor>,
    history: Arc<RwLock<Vec<goval::OtPacket>>>,
    watcher: FSWatcher,
    _sending_map: Arc<RwLock<HashMap<i32, tokio::sync::mpsc::UnboundedSender<IPCMessage>>>>,
}

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{client::ClientInfo, fs_watcher::FSWatcher, FSEvent, IPCMessage};

use super::traits;
use anyhow::{format_err, Result};
use async_trait::async_trait;
use log::{as_debug, debug, error, warn, trace};
use similar::TextDiff;
use tokio::{
    fs,
    sync::{broadcast::error::RecvError, RwLock},
};

enum LoopControl {
    Cont(Result<()>),
    Break,
}

impl OT {
    pub async fn new() -> Result<OT> {
        let watcher = FSWatcher::new().await?;

        let chan = OT {
            crc32: Arc::new(RwLock::new(0)),
            version: Arc::new(RwLock::new(1)),
            contents: Arc::new(RwLock::new("".into())),
            path: "".to_string(),
            cursors: HashMap::new(),
            history: Arc::new(RwLock::new(vec![])),
            watcher,
            _sending_map: Arc::new(RwLock::new(HashMap::new())),
        };

        Ok(chan)
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

                let mut _crc32 = self.crc32.write().await;
                *_crc32 = crc32;
                drop(_crc32);

                let mut contents = self.contents.write().await;
                *contents = String::from_utf8(byte_contents.clone())?.into();
                drop(contents);

                let timestamp = Some(prost_types::Timestamp {
                    seconds: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                    nanos: 0,
                });

                let version = self.version.read().await.clone();

                let hist_item = goval::OtPacket {
                    spooky_version: version,
                    version,
                    op: vec![],
                    crc32,
                    committed: timestamp,
                    author: goval::ot_packet::Author::User.into(),
                    user_id: 23352071,
                    nonce: 0,
                };

                let mut history = self.history.write().await;
                history.push(hist_item);
                drop(history);

                let mut link_response = goval::Command::default();

                let mut file = goval::File::default();
                file.path = path.clone();
                file.content = byte_contents;

                let _inner = goval::OtLinkFileResponse {
                    version,
                    linked_file: Some(file),
                };
                link_response.body = Some(goval::command::Body::OtLinkFileResponse(_inner));

                self.watcher.watch(vec![path]).await?;

                let mut reader = self.watcher.get_event_reader().await;
                let sending_map = self._sending_map.clone();
                let file_path = self.path.clone();
                let crc32 = self.crc32.clone();
                let contents = self.contents.clone();
                let version = self.version.clone();
                let history = self.history.clone();
                let channel_id = info.id.clone();
                tokio::spawn(async move {
                    loop {
                        let res = async {
                            match reader.recv().await {
                                Ok(res) => {
                                    trace!(event = as_debug!(res.clone()), file_path = file_path; "oooh event");
                                    match res {
                                        FSEvent::Modify(path) => {
                                            trace!(condition = (path == file_path), path = path, file_path = file_path; "Conditional time");
                                            if path == file_path {
                                                let new_contents = match fs::read(&path).await {
                                                    Ok(contents) => contents,
                                                    Err(err) => {
                                                        return LoopControl::Cont(Err(err.into()))
                                                    }
                                                };

                                                let new_crc32 = crc32fast::hash(&new_contents);
                                                trace!("Awaiting crc32 lock");
                                                let old_crc32 = crc32.read().await;
                                                if new_crc32 == old_crc32.clone() {
                                                    return LoopControl::Cont(Ok(()));
                                                }

                                                drop(old_crc32);
                                                
                                                trace!("Awaiting contents lock");
                                                let mut old_contents = contents.write().await;
                                                trace!("Awaiting version lock");
                                                let mut version = version.write().await;
                                                *version += 1;

                                                let new_version = version.clone();
                                                let new_contents = String::from_utf8(new_contents)
                                                    .expect("TODO: Deal with this");

                                                let ops = diff(
                                                    old_contents.to_string(),
                                                    new_contents.clone(),
                                                );

                                                *old_contents = new_contents.into();

                                                let packet = goval::OtPacket {
                                                    spooky_version: new_version,
                                                    version: new_version,
                                                    op: ops,
                                                    committed: None,
                                                    crc32: new_crc32,
                                                    nonce: 0,
                                                    user_id: 0,
                                                    author: goval::ot_packet::Author::User.into(),
                                                };

                                                trace!("Awaiting history lock");
                                                let mut history = history.write().await;
                                                history.push(packet.clone());

                                                let mut ot_notif = goval::Command::default();
                                                ot_notif.channel = channel_id;
                                                ot_notif.body =
                                                    Some(goval::command::Body::Ot(packet));

                                                // info.send(ot_notif, crate::SendSessions::Everyone)
                                                //     .await
                                                //     .expect("TODO: Deal with this");

                                                trace!("Awaiting sending lock");
                                                let to_send = sending_map.read().await;

                                                trace!("Sending...");
                                                for (session, sender) in to_send.iter() {
                                                    trace!(session = session; "Awaiting send");
                                                    let mut msg = ot_notif.clone();
                                                    msg.session = session.clone();

                                                    sender
                                                        .send(IPCMessage {
                                                            command: msg,
                                                            session: session.clone(),
                                                        })
                                                        .expect("TODO: deal with this");
                                                    trace!(session = session; "Sent");
                                                }
                                            }
                                        }
                                        FSEvent::Err(err) => {
                                            error!(error = err; "Error in FS event listener")
                                        }
                                        _ => {
                                            debug!(message = as_debug!(res); "Ignoing FS event")
                                        }
                                    }

                                    LoopControl::Cont(Ok(()))
                                }
                                Err(err) => match err {
                                    RecvError::Closed => LoopControl::Break,
                                    RecvError::Lagged(ammount) => {
                                        warn!(messages = ammount; "FSEvents lagged");
                                        LoopControl::Cont(Ok(()))
                                    }
                                },
                            }
                        }
                        .await;

                        match res {
                            LoopControl::Break => break,
                            LoopControl::Cont(result) => result.expect("TODO: deal with this"),
                        }
                    }
                });

                return Ok(Some(link_response));
            } else {
                return Err(format_err!("Command sent before otLinkFile"));
            }
        }

        match body {
            goval::command::Body::Ot(ot) => {
                let mut cursor: usize = 0;

                let mut contents = self.contents.write().await;
                for op in ot.op.clone() {
                    match op.op_component.unwrap() {
                        goval::ot_op_component::OpComponent::Skip(_skip) => {
                            let skip: usize = _skip.try_into()?;
                            if skip + cursor > contents.len_chars() {
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
                            if delete + cursor > contents.len_chars() {
                                let mut err = goval::Command::default();
                                err.body = Some(goval::command::Body::Error(
                                    "Invalid delete past bounds".to_string(),
                                ));
                                return Ok(Some(err));
                            }

                            contents.remove(cursor..(cursor + delete))
                        }
                        goval::ot_op_component::OpComponent::Insert(insert) => {
                            contents.insert(cursor, &insert)
                        }
                    }
                }

                let to_write = contents.to_string();
                let mut version = self.version.write().await;
                *version += 1;
                let saved_version = version.clone();
                drop(version);
                let user_id = 22261053;
                let crc32 = crc32fast::hash(to_write.as_bytes());

                let packet = goval::OtPacket {
                    spooky_version: saved_version,
                    version: saved_version,
                    op: ot.op,
                    committed: None,
                    crc32,
                    nonce: 0,
                    user_id,
                    author: ot.author,
                };

                let mut history = self.history.write().await;
                history.push(packet.clone());
                drop(history);

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
        session: i32,
        sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<Option<goval::Command>> {
        let mut sending_map = self._sending_map.write().await;
        sending_map.insert(session, sender);
        drop(sending_map);

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
            contents: self.contents.read().await.to_string(),
            version: self.version.read().await.clone(),
            linked_file: Some(file),
            cursors: cursors,
        };
        status.body = Some(goval::command::Body::Otstatus(_inner));

        Ok(Some(status))
    }

    async fn shutdown(self: Box<OT>, _info: &super::types::ChannelInfo) -> Result<()> {
        self.watcher.shutdown().await;
        Ok(())
    }
}

fn diff(old_text: String, new_text: String) -> Vec<goval::OtOpComponent> {
    let mut _differ = TextDiff::configure();
    let differ = _differ.timeout(Duration::from_secs(1));
    let diff = differ.diff_chars(&old_text, &new_text);

    let mut parts: Vec<goval::OtOpComponent> = vec![];
    let mut last_op: Option<goval::ot_op_component::OpComponent> = None;
    for part in diff.iter_all_changes() {
        let mut new_op: Option<goval::ot_op_component::OpComponent> = None;
        match part.tag() {
            similar::ChangeTag::Equal => {
                if let Some(goval::ot_op_component::OpComponent::Skip(amount)) = last_op.clone() {
                    last_op = Some(goval::ot_op_component::OpComponent::Skip(
                        amount + part.value().len() as u32,
                    ))
                } else {
                    new_op = Some(goval::ot_op_component::OpComponent::Skip(
                        part.value().len() as u32,
                    ));
                }
            }
            similar::ChangeTag::Delete => {
                if let Some(goval::ot_op_component::OpComponent::Delete(amount)) = last_op.clone() {
                    last_op = Some(goval::ot_op_component::OpComponent::Delete(
                        amount + part.value().len() as u32,
                    ))
                } else {
                    new_op = Some(goval::ot_op_component::OpComponent::Delete(
                        part.value().len() as u32,
                    ));
                }
            }
            similar::ChangeTag::Insert => {
                if let Some(goval::ot_op_component::OpComponent::Insert(same)) = last_op.clone() {
                    last_op = Some(goval::ot_op_component::OpComponent::Insert(
                        same + part.value(),
                    ))
                } else {
                    new_op = Some(goval::ot_op_component::OpComponent::Insert(
                        part.value().to_string(),
                    ));
                }
            }
        }

        if let Some(new_part) = new_op {
            if let Some(last_part) = last_op.clone() {
                parts.push(goval::OtOpComponent {
                    op_component: Some(last_part),
                });
            }

            last_op = Some(new_part);
        }
    }

    if let Some(op) = last_op {
        match op {
            goval::ot_op_component::OpComponent::Skip(_) => {}
            _ => parts.push(goval::OtOpComponent {
                op_component: Some(op),
            }),
        }
    }

    parts
}

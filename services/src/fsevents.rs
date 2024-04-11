pub struct FSEvents {
    // watching: Vec<String>
    watcher: FSWatcher,
}

use super::traits;
use crate::{FSEvent, FSWatcher};
use anyhow::{format_err, Result};
use goval::{Command, File, FileEvent};
use tracing::{error, trace, warn};

impl FSEvents {
    pub async fn new(
        sender: tokio::sync::mpsc::UnboundedSender<crate::ChannelMessage>,
    ) -> Result<FSEvents> {
        let watcher = FSWatcher::new(sender).await?;

        let chan = FSEvents {
            // watching: vec![],
            watcher,
        };

        Ok(chan)
    }
}

#[async_trait::async_trait]
impl traits::Service for FSEvents {
    async fn open(&mut self, _info: &super::types::ChannelInfo) -> Result<()> {
        Ok(())
    }

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
            goval::command::Body::SubscribeFile(subscribe) => {
                let mut files = vec![];
                for file in subscribe.files {
                    files.push(file.path);
                }

                self.watcher.watch(files).await?;
                Ok(Some(goval::Command {
                    body: Some(goval::command::Body::Ok(goval::Ok {})),
                    ..Default::default()
                }))
            }
            _ => {
                warn!(cmd = ?message, "Unknown fs event command");
                Ok(None)
            }
        }
    }

    async fn fsevent(&mut self, info: &super::types::ChannelInfo, event: FSEvent) -> Result<()> {
        trace!(?event, "fs event");
        let file;
        let dest;
        let op;
        match event {
            FSEvent::Modify(path) => {
                file = Some(File {
                    path,
                    ..Default::default()
                });
                dest = None;
                op = goval::file_event::Op::Modify;
            }
            FSEvent::Create(path) => {
                file = Some(File {
                    path,
                    ..Default::default()
                });
                dest = None;
                op = goval::file_event::Op::Create;
            }
            FSEvent::Remove(path) => {
                file = Some(File {
                    path,
                    ..Default::default()
                });
                dest = None;
                op = goval::file_event::Op::Remove;
            }
            FSEvent::Rename(old_path, new_path) => {
                file = Some(File {
                    path: new_path,
                    ..Default::default()
                });
                dest = Some(File {
                    path: old_path,
                    ..Default::default()
                });
                op = goval::file_event::Op::Move;
            }
            FSEvent::Err(err) => {
                error!(err, "Error in FS event listener");
                return Err(format_err!("{err}"));
            }
        };

        info.send(
            Command {
                body: Some(goval::command::Body::FileEvent(FileEvent {
                    file,
                    dest,
                    op: op.into(),
                })),
                ..Default::default()
            },
            crate::SendSessions::Everyone,
        )
        .await?;
        Ok(())
    }

    async fn shutdown(self: Box<FSEvents>, _info: &super::types::ChannelInfo) -> Result<()> {
        self.watcher.shutdown().await;
        Ok(())
    }
}

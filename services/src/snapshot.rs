pub struct Snapshot {}

use super::traits;
use anyhow::{format_err, Result};
use async_trait::async_trait;

#[async_trait]
impl traits::Service for Snapshot {
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
            goval::command::Body::FsSnapshot(_) => {
                let ok = goval::Command {
                    body: Some(goval::command::Body::Ok(goval::Ok {})),
                    ..Default::default()
                };
                Ok(Some(ok))
            }
            _ => Ok(None),
        }
    }
}

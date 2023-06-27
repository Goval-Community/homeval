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
                let mut ok = goval::Command::default();
                ok.body = Some(goval::command::Body::Ok(goval::Ok {}));
                Ok(Some(ok))
            }
            _ => Ok(None),
        }
    }
}

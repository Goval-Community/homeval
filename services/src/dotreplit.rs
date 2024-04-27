pub struct DotReplit {}
use crate::SessionID;

use super::traits;

use anyhow::{format_err, Result};
use async_trait::async_trait;

#[async_trait]
impl traits::Service for DotReplit {
    async fn message(
        &mut self,
        _info: &super::types::ChannelInfo,
        message: goval::Command,
        _session: SessionID,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };

        match body {
            goval::command::Body::DotReplitGetRequest(_) => {
                let mut dotreplit = goval::Command::default();
                let inner: goval::DotReplit = (&*_info.dotreplit.read().await).into();

                dotreplit.body = Some(goval::command::Body::DotReplitGetResponse(
                    goval::DotReplitGetResponse {
                        dot_replit: Some(inner),
                    },
                ));

                Ok(Some(dotreplit))
            }
            _ => Ok(None),
        }
    }
}

pub struct Chat {}
use crate::{ClientInfo, SendSessions};

use super::traits;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
impl traits::Service for Chat {
    async fn open(&mut self, _info: &super::types::ChannelInfo) -> Result<()> {
        Ok(())
    }

    async fn message(
        &mut self,
        info: &super::types::ChannelInfo,
        message: goval::Command,
        session: i32,
    ) -> Result<Option<goval::Command>> {
        info.send(message, SendSessions::EveryoneExcept(session))
            .await?;
        Ok(None)
    }
}

use anyhow::Result;
use async_trait::async_trait;

use crate::ClientInfo;

#[async_trait]
pub(crate) trait Service {
    async fn open(&mut self, _info: &super::types::ChannelInfo) -> Result<()> {
        Ok(())
    }
    async fn shutdown(&mut self, _info: &super::types::ChannelInfo) -> Result<()> {
        Ok(())
    }

    async fn message(
        &mut self,
        _info: &super::types::ChannelInfo,
        _message: goval::Command,
        _session: i32,
    ) -> Result<Option<goval::Command>> {
        Ok(None)
    }

    async fn attach(
        &mut self,
        _info: &super::types::ChannelInfo,
        _client: ClientInfo,
        _session: i32,
    ) -> Result<Option<goval::Command>> {
        Ok(None)
    }

    async fn detach(&mut self, _info: &super::types::ChannelInfo, _session: i32) -> Result<()> {
        Ok(())
    }
}

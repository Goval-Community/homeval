use anyhow::Result;
use async_trait::async_trait;

use crate::{ClientInfo, FSEvent, IPCMessage};

#[async_trait]
pub(crate) trait Service {
    async fn open(&mut self, _info: &super::types::ChannelInfo) -> Result<()> {
        Ok(())
    }
    async fn shutdown(self: Box<Self>, _info: &super::types::ChannelInfo) -> Result<()> {
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

    async fn proccess_died(
        &mut self,
        _info: &super::types::ChannelInfo,
        _exit_code: i32,
    ) -> Result<()> {
        Ok(())
    }

    async fn fsevent(&mut self, _info: &super::types::ChannelInfo, _event: FSEvent) -> Result<()> {
        Ok(())
    }

    async fn attach(
        &mut self,
        _info: &super::types::ChannelInfo,
        _client: ClientInfo,
        _session: i32,
        _sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<Option<goval::Command>> {
        Ok(None)
    }

    async fn detach(&mut self, _info: &super::types::ChannelInfo, _session: i32) -> Result<()> {
        Ok(())
    }
}

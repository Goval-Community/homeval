use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use crate::{ClientInfo, FSEvent, IPCMessage, ReplspaceMessage, SessionID};

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
        _session: SessionID,
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

    async fn replspace(
        &mut self,
        _info: &super::types::ChannelInfo,
        _msg: ReplspaceMessage,
        _session: SessionID,
        _respond: Option<Sender<ReplspaceMessage>>,
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
        _session: SessionID,
        _sender: tokio::sync::mpsc::UnboundedSender<IPCMessage>,
    ) -> Result<Option<goval::Command>> {
        Ok(None)
    }

    async fn detach(
        &mut self,
        _info: &super::types::ChannelInfo,
        _session: SessionID,
    ) -> Result<()> {
        Ok(())
    }
}

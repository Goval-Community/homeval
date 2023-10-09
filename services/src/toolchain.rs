pub struct Toolchain {}
use super::traits;
use async_trait::async_trait;
use log::{as_debug, debug};

use anyhow::{format_err, Result};

#[async_trait]
impl traits::Service for Toolchain {
    async fn message(
        &mut self,
        _info: &super::types::ChannelInfo, // TODO: use this to give real toolchain info
        message: goval::Command,
        _session: i32,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };
        match body {
            goval::command::Body::NixModulesGetRequest(_) => {
                let mut modules = goval::Command::default();

                modules.body = Some(goval::command::Body::NixModulesGetResponse(
                    goval::NixModulesGetResponse::default(),
                ));

                Ok(Some(modules))
            }
            goval::command::Body::ToolchainGetRequest(_) => {
                let mut toolchain = goval::Command::default();

                let mut inner = goval::ToolchainGetResponse::default();
                let mut configs = goval::ToolchainConfigs::default();
                configs.runs = vec![goval::RunOption {
                    id: "homeval/test".into(),
                    name: "Test".into(),
                    file_param: false,
                    language: "idk".into(),
                    file_type_attrs: None,
                    interpreter: false,
                    optional_file_param: false,
                }];
                inner.configs = Some(configs);
                toolchain.body = Some(goval::command::Body::ToolchainGetResponse(inner));

                Ok(Some(toolchain))
            }
            _ => {
                debug!(msg = as_debug!(message); "Unrecognized command :/");
                Ok(None)
            }
        }
    }
}
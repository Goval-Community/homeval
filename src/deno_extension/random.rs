use deno_core::{op, OpDecl};
use serde::{Deserialize, Serialize};

use deno_core::error::AnyError;

use log::{log, warn};

use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[op]
fn op_time_milliseconds() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0)); // Timestamp set to 0 if current time is before unix epoch
    timestamp.as_millis().to_string()
}

#[op(deferred)]
pub async fn op_sleep(millis: u64) -> Result<(), AnyError> {
    tokio::time::sleep(Duration::from_millis(millis)).await;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsoleLogLevels {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl ConsoleLogLevels {
    pub fn to_log(&self) -> log::Level {
        match self {
            ConsoleLogLevels::Trace => log::Level::Trace,
            ConsoleLogLevels::Debug => log::Level::Debug,
            ConsoleLogLevels::Info => log::Level::Info,
            ConsoleLogLevels::Warn => log::Level::Warn,
            ConsoleLogLevels::Error => log::Level::Error,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub service: String,
    pub name: Option<String>,
    pub id: i32,
}

#[op]
fn op_console_log(
    level: ConsoleLogLevels,
    service: Service,
    message: String,
) -> Result<(), AnyError> {
    let mut name = "".to_string();
    match service.name {
        None => {}
        Some(_name) => {
            name = format!(":{}", _name);
        }
    }
    let target = &format!("homeval/v8: {}{}", service.service, name);

    log!(
        target: target,
        level.to_log(),
        "{}",
        message.replace("\\n", "\n")
    );
    Ok(())
}

#[op]
fn op_get_env_var(key: String) -> Result<Option<String>, AnyError> {
    match std::env::var(key.clone()) {
        Ok(val) => Ok(Some(val)),
        Err(e) => {
            warn!("Error occured while fetching env var: '{}' {}", key, e);
            Ok(None)
        }
    }
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        // Time
        op_time_milliseconds::decl(),
        op_sleep::decl(),
        // Console
        op_console_log::decl(),
        // Env
        op_get_env_var::decl(),
    ]
}

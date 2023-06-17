use crate::config::dotreplit::DotReplit;
use deno_core::{error::AnyError, op, OpDecl};

#[op]
fn op_server_name() -> Result<String, AnyError> {
    Ok(env!("CARGO_PKG_NAME").to_string())
}

#[op]
fn op_server_version() -> Result<String, AnyError> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[op]
fn op_server_license() -> Result<String, AnyError> {
    Ok(env!("CARGO_PKG_LICENSE").to_string())
}

#[op]
fn op_server_authors() -> Result<String, AnyError> {
    Ok(env!("CARGO_PKG_AUTHORS").to_string())
}

#[op]
fn op_server_repository() -> Result<String, AnyError> {
    Ok(env!("CARGO_PKG_REPOSITORY").to_string())
}

#[op]
fn op_server_description() -> Result<String, AnyError> {
    Ok(env!("CARGO_PKG_DESCRIPTION").to_string())
}

#[op]
fn op_server_uptime() -> Result<u64, AnyError> {
    Ok(crate::START_TIME.elapsed().as_secs())
}

#[op]
fn op_get_supported_services() -> Result<Vec<String>, AnyError> {
    Ok(crate::IMPLEMENTED_SERVICES.clone())
}

#[op]
fn op_get_dotreplit_config() -> Result<DotReplit, AnyError> {
    Ok(crate::DOTREPLIT_CONFIG.clone())
}

#[op]
fn op_get_running_os() -> Result<String, AnyError> {
    Ok(std::env::consts::OS.to_string())
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_server_name::decl(),
        op_server_version::decl(),
        op_server_license::decl(),
        op_server_authors::decl(),
        op_server_repository::decl(),
        op_server_description::decl(),
        op_server_uptime::decl(),
        op_get_supported_services::decl(),
        // Config
        op_get_dotreplit_config::decl(),
        // System info
        op_get_running_os::decl(),
    ]
}

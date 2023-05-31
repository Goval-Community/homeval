use deno_core::{error::AnyError, op, OpDecl};
use log::error;
use serde::{Deserialize, Serialize};
use std::io::Error;
use sysinfo::{self, CpuRefreshKind, RefreshKind, SystemExt};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    // pub path: String,
    // pub r#type: FileType,
}

#[op]
fn op_cpu_info(session: i32) -> Result<CpuInfo, AnyError> {
    let refresh = RefreshKind::new();
    let mut system =
        sysinfo::System::new_with_specifics(refresh.with_cpu(CpuRefreshKind::everything()));

    system.refresh_cpu();
    // system.global_cpu_info().time;
    let _read = crate::SESSION_CLIENT_INFO.read(&session);
    if !_read.contains_key() {
        return Ok(CpuInfo {});
    }

    match _read.get() {
        Some(info) => Ok(CpuInfo {}),
        None => Ok(CpuInfo {}),
    }
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![op_cpu_info::decl()]
}

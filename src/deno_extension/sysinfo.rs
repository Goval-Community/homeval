use deno_core::{error::AnyError, op, OpDecl};
use serde::Serialize;
// use sysinfo::{self, CpuRefreshKind, RefreshKind, SystemExt};
use systemstat::{Platform, System};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {}

#[op]
async fn op_cpu_info() -> Result<String, AnyError> {
    tokio::task::spawn_blocking(move || Ok(crate::CPU_STATS.elapsed().as_nanos().to_string()))
        .await?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    total: u64,
    free: u64,
}

#[op]
async fn op_memory_info() -> Result<MemoryInfo, AnyError> {
    let info = tokio::task::spawn_blocking(move || -> std::io::Result<MemoryInfo> {
        match System::new().memory() {
            Ok(mem) => Ok(MemoryInfo {
                total: mem.total.as_u64(),
                free: mem.free.as_u64(),
            }),
            Err(_) => Ok(MemoryInfo { total: 0, free: 0 }),
        }
    })
    .await??;

    Ok(info)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    available: u64,
    total: u64,
    free: u64,
}

#[op]
async fn op_disk_info() -> Result<DiskInfo, AnyError> {
    let info = tokio::task::spawn_blocking(move || -> std::io::Result<DiskInfo> {
        match System::new().mount_at("/") {
            Ok(disk) => {
                let mut info = DiskInfo {
                    available: 0,
                    total: 0,
                    free: 0,
                };

                info.available += disk.avail.as_u64();
                info.total += disk.total.as_u64();
                info.free += disk.free.as_u64();

                Ok(info)
            }
            Err(_) => Ok(DiskInfo {
                available: 0,
                total: 0,
                free: 0,
            }),
        }
    })
    .await??;

    Ok(info)
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_cpu_info::decl(),
        op_memory_info::decl(),
        op_disk_info::decl(),
    ]
}

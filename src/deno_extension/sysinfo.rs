use deno_core::{error::AnyError, op, OpDecl};
use log::trace;
use serde::Serialize;
use sysinfo::{self, CpuRefreshKind, DiskExt, RefreshKind, SystemExt};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    // pub path: String,
    // pub r#type: FileType,
}

#[op]
fn op_cpu_info() -> Result<CpuInfo, AnyError> {
    let refresh = RefreshKind::new();
    let mut system =
        sysinfo::System::new_with_specifics(refresh.with_cpu(CpuRefreshKind::everything()));

    system.refresh_cpu();

    Ok(CpuInfo {})
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
    let refresh = RefreshKind::new();
    let mut system = sysinfo::System::new_with_specifics(refresh.with_disks());

    system = tokio::task::spawn_blocking(move || {
        system.refresh_disks();
        system
    })
    .await?;

    let info = tokio::task::spawn_blocking(move || {
        let disks = system.disks();

        trace!("Disks: {:#?}", disks);

        let mut info = DiskInfo {
            available: 0,
            total: 0,
            free: 0,
        };

        for disk in disks {
            info.total += disk.total_space();
            info.available += disk.available_space();
        }

        info.free = info.total - info.available;

        info
    })
    .await?;

    Ok(info)
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![op_cpu_info::decl(), op_disk_info::decl()]
}

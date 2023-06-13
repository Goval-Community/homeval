use deno_core::{Extension, OpDecl};

pub mod cmd;
pub mod diff;
pub mod fs;
pub mod fs_events;
pub mod messaging;
pub mod pty;
pub mod quick_cmd;
pub mod random;
pub mod server_info;
pub mod sysinfo;

#[cfg(feature = "database")]
pub mod database_real;
#[cfg(feature = "database")]
use database_real as database;

#[cfg(not(feature = "database"))]
pub mod database_faux;
#[cfg(not(feature = "database"))]
use database_faux as database;

pub use messaging::JsMessage;
pub use random::Service;

pub fn make_extension() -> Extension {
    let mut ops: Vec<OpDecl> = vec![];

    // Add extension op decls
    ops.append(&mut cmd::get_op_decls());
    ops.append(&mut diff::get_op_decls());
    ops.append(&mut fs::get_op_decls());
    ops.append(&mut fs_events::get_op_decls());
    ops.append(&mut messaging::get_op_decls());
    ops.append(&mut pty::get_op_decls());
    ops.append(&mut quick_cmd::get_op_decls());
    ops.append(&mut random::get_op_decls());
    ops.append(&mut server_info::get_op_decls());
    ops.append(&mut sysinfo::get_op_decls());
    ops.append(&mut database::get_op_decls());

    Extension::builder("homeval").ops(ops).build()
}

use deno_core::{Extension, OpDecl};

pub mod cmd;
pub mod fs;
pub mod fs_events;
pub mod messaging;
pub mod pty;
pub mod quick_cmd;
pub mod random;
pub mod sysinfo;

pub use messaging::JsMessage;
pub use random::Service;

pub fn make_extension() -> Extension {
    let mut ops: Vec<OpDecl> = vec![];

    // Add extension op decls
    ops.append(&mut fs::get_op_decls());
    ops.append(&mut messaging::get_op_decls());
    ops.append(&mut random::get_op_decls());
    ops.append(&mut pty::get_op_decls());
    ops.append(&mut cmd::get_op_decls());
    ops.append(&mut quick_cmd::get_op_decls());
    ops.append(&mut fs_events::get_op_decls());
    ops.append(&mut sysinfo::get_op_decls());

    Extension::builder().ops(ops).build()
}

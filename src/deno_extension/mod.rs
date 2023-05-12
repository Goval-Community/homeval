use deno_core::{Extension, OpDecl};

pub mod fs;
pub mod messaging;
pub mod random;

pub use messaging::JsMessage;
pub use random::Service;

pub fn make_extension() -> Extension {
    let mut ops: Vec<OpDecl> = vec![];
    ops.append(&mut fs::get_op_decls());
    ops.append(&mut messaging::get_op_decls());
    ops.append(&mut random::get_op_decls());
    Extension::builder().ops(ops).build()
}

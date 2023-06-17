use deno_core::{error::AnyError, op, OpDecl};
use std::io::Error;

#[op]
fn op_database_exists() -> Result<bool, AnyError> {
    Ok(false)
}

// Any function using this should never be called ever, since op_database_exists() returns false
macro_rules! db_disabled {
    () => {
        Err(Error::new(std::io::ErrorKind::NotConnected, "database is disabled").into())
    };
}

#[op]
async fn op_database_get_file() -> Result<Option<()>, AnyError> {
    db_disabled!()
}

#[op]
async fn op_database_set_file(_model: ()) -> Result<(), AnyError> {
    db_disabled!()
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_database_exists::decl(),
        op_database_get_file::decl(),
        op_database_set_file::decl(),
    ]
}

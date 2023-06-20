use std::collections::HashMap;

use deno_core::{error::AnyError, op, OpDecl};
use serde::{Deserialize, Serialize};
use tokio::sync::{OnceCell, RwLock};

#[derive(Clone, Serialize, Deserialize)]
pub struct File {
    pub name: String,
    pub crc32: i32,
    pub contents: String,
    pub history: Vec<String>,
}

static FILES: OnceCell<RwLock<HashMap<String, File>>> = OnceCell::const_new();

#[op]
fn op_database_exists() -> Result<bool, AnyError> {
    Ok(true)
}

#[op]
async fn op_database_get_file(file_name: String) -> Result<Option<File>, AnyError> {
    match FILES
        .get_or_init(|| async { RwLock::new(HashMap::new()) })
        .await
        .read()
        .await
        .get(&file_name)
    {
        Some(file) => Ok(Some(file.clone())),
        None => Ok(None),
    }
}

#[op]
async fn op_database_set_file(file: File) -> Result<(), AnyError> {
    FILES
        .get_or_init(|| async { RwLock::new(HashMap::new()) })
        .await
        .write()
        .await
        .insert(file.name.clone(), file);
    Ok(())
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_database_exists::decl(),
        op_database_get_file::decl(),
        op_database_set_file::decl(),
    ]
}

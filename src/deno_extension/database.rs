use deno_core::{error::AnyError, op, OpDecl};
use sea_orm::entity::prelude::*;
// use sea_orm::prelude::*;
use sea_orm::DeriveEntityModel;
use serde::{Deserialize, Serialize};
/*

spookyVersion: this.version,
op: [{insert: this.contents}],
crc32: CRC32.str(this.contents),
committed: {
    seconds: (Date.now()/ 1000n).toString(),
    nanos: 0
},
version: this.version,
author: api.OTPacket.Author.USER,
// use https://replit.com/@homeval for initial insert
userId: 20567961
*/

// #[derive(Clone, Debug, PartialEq, Eq, DeriveActiveEnum, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// #[sea_orm]
// enum DatabaseFileHistoryOp {
//     #[sea_orm(string_value = "insert")]
//     Insert(String),
//     #[sea_orm(string_value = "delete")]
//     Delete(u32),
//     #[sea_orm(string_value = "skip")]
//     Skip(u32),
// }

// #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)] // DeriveEntity,
// #[serde(rename_all = "camelCase")]
// struct DatabaseFileHistoryItem {
//     version: i32,
//     author: i8,
//     user_id: i32,
//     commited: i64,
//     crc32: u32,
//     op: Vec<DatabaseFileHistoryOp>,
// }

// #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
// // #[serde(rename_all = "camelCase")]
// #[sea_orm(table_name = "files")]
// struct Model {
//     #[sea_orm(primary_key)]
//     pub id: i32,
//     pub name: String,
//     // #[sea_orm(primary_key)]
//     // // file_name: String,
//     // last_crc32: u32,
//     // history: Vec<DatabaseFileHistoryItem>,
//     // version: u32,
// }

// #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
// enum Relation {
//     // #[sea_orm(has_many = "super::fruit::Entity")]
//     // Fruit,
// }

#[op]
async fn op_database_exists() -> Result<bool, AnyError> {
    Ok(crate::DATABASE_CONNECTION.get().await.is_some())
}

#[op]
async fn op_database_files_get(filename: String) -> Result<Option<i32>, AnyError> {
    Ok(None)
}

#[op]
async fn op_database_files_set(
    filename: String,
    file_info: i32,
    exists: bool,
) -> Result<(), AnyError> {
    Ok(())
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_database_exists::decl(),
        op_database_files_get::decl(),
        op_database_files_set::decl(),
    ]
}

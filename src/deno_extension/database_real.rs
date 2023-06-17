use std::io::Error;

use deno_core::{error::AnyError, op, OpDecl};
use entity::files;
use log::trace;
use sea_orm::entity::EntityTrait;
use sea_query::OnConflict;

#[op]
fn op_database_exists() -> Result<bool, AnyError> {
    match crate::DATABASE.get() {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

#[op]
async fn op_database_get_file(file_name: String) -> Result<Option<files::Model>, AnyError> {
    match crate::DATABASE.get() {
        Some(db) => Ok(files::Entity::find_by_id(file_name).one(db).await?),
        None => Err(Error::new(
            std::io::ErrorKind::NotConnected,
            "No database connection found",
        )
        .into()),
    }
}

#[op]
async fn op_database_set_file(model: files::Model) -> Result<(), AnyError> {
    match crate::DATABASE.get() {
        Some(db) => {
            trace!("Inserting file to db: {:#?}", model);
            let active: files::ActiveModel = model.into();
            files::Entity::insert(active)
                .on_conflict(
                    OnConflict::column(files::Column::Name)
                        .update_columns([
                            files::Column::Contents,
                            files::Column::Crc32,
                            files::Column::History,
                        ])
                        .to_owned(),
                )
                .exec(db)
                .await?;
            Ok(())
        }
        None => Err(Error::new(
            std::io::ErrorKind::NotConnected,
            "No database connection found",
        )
        .into()),
    }
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_database_exists::decl(),
        op_database_get_file::decl(),
        op_database_set_file::decl(),
    ]
}

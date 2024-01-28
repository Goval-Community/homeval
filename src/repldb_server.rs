use anyhow::Result;
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    routing::{delete, get, post},
    Form, Router,
};
use entity::repldb;
use log::{as_error, error, info, warn};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sea_query::OnConflict;
use serde::Deserialize;
use std::{collections::HashMap, net::SocketAddr};

pub async fn start_server() -> Result<()> {
    if crate::DATABASE.get().is_none() {
        warn!("Database missing, disabling repldb server.");
        return Ok(());
    }

    let app = Router::new()
        .route("/", post(set_value))
        .route("/", get(list_keys))
        .route("/:key", get(get_value))
        .route("/:key", delete(delete_value));

    let listener = if let Ok(addr) = std::env::var("HOMEVAL_REPLDB_ADDR") {
        tokio::net::TcpListener::bind(addr.parse::<SocketAddr>()?).await?
    } else {
        tokio::net::TcpListener::bind("127.0.0.1:0").await?
    };

    let host = format!("127.0.0.1:{}", listener.local_addr()?);

    info!("ReplDB server listening on: {}", host);
    crate::CHILD_PROCS_ENV_BASE
        .write()
        .await
        .insert("REPLIT_DB_URL".to_string(), host);

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

async fn set_value(Form(data): Form<HashMap<String, String>>) -> StatusCode {
    let database = crate::DATABASE
        .get()
        .expect("DATABASE is known to be set or else repldb server is disabled");

    for (key, value) in data.iter() {
        let active: repldb::ActiveModel = repldb::ActiveModel {
            key: sea_orm::ActiveValue::Set(key.clone()),
            value: sea_orm::ActiveValue::Set(value.clone()),
        };

        let result = repldb::Entity::insert(active)
            .on_conflict(
                OnConflict::column(repldb::Column::Key)
                    .update_columns([repldb::Column::Value])
                    .to_owned(),
            )
            .exec(database)
            .await;

        match result {
            Ok(_) => {}
            Err(err) => {
                error!(error = as_error!(err); "Encountered error inserting key into database");
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        }
    }

    StatusCode::OK
}

async fn get_value(Path(key): Path<String>) -> (StatusCode, String) {
    let database = crate::DATABASE
        .get()
        .expect("DATABASE is known to be set or else repldb server is disabled");

    let result = repldb::Entity::find_by_id(key).one(database).await;

    match result {
        Ok(value) => match value {
            None => (StatusCode::NOT_FOUND, "".to_string()),
            Some(data) => (StatusCode::OK, data.value),
        },
        Err(err) => {
            error!(error = as_error!(err); "Encountered error reading key from database");
            (StatusCode::INTERNAL_SERVER_ERROR, "".to_string())
        }
    }
}

async fn delete_value(Path(key): Path<String>) -> StatusCode {
    let database = crate::DATABASE
        .get()
        .expect("DATABASE is known to be set or else repldb server is disabled");

    let result = repldb::Entity::delete_by_id(key).exec(database).await;

    match result {
        Ok(value) => {
            if value.rows_affected == 0 {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::OK
            }
        }
        Err(err) => {
            error!(error = as_error!(err); "Encountered error deleting key from database");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[derive(Deserialize)]
struct ListKeys {
    prefix: Option<String>,
}

async fn list_keys(Query(__prefix): Query<ListKeys>) -> (StatusCode, String) {
    let prefix = match __prefix.prefix {
        Some(prefix) => prefix,
        None => return (StatusCode::OK, "".to_string()),
    };

    let database = crate::DATABASE
        .get()
        .expect("DATABASE is known to be set or else repldb server is disabled");

    let result = repldb::Entity::find()
        .filter(repldb::Column::Key.starts_with(&prefix))
        .all(database)
        .await;

    match result {
        Ok(value) => {
            let mut keys = "".to_string();

            for (index, info) in value.iter().enumerate() {
                if index != 0 {
                    keys += "\n"
                }
                keys += &info.key
            }

            (StatusCode::OK, keys)
        }
        Err(err) => {
            error!(error = as_error!(err); "Encountered error listing keys in database");
            (StatusCode::INTERNAL_SERVER_ERROR, "".to_string())
        }
    }
}

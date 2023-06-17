use axum::{
    extract::Query,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use deno_core::error::AnyError;
use log::{error, info};
use serde::{Deserialize, Serialize};
use textnonce::TextNonce;
use tokio::sync::oneshot::channel;

use crate::deno_extension::{messaging::ReplspaceMessage, JsMessage};

pub async fn start_server() -> Result<(), AnyError> {
    info!("Replspace api server listening on: 127.0.0.1:8283");
    let app = Router::new()
        .route("/files/open", post(open_file))
        .route("/github/token", get(get_gh_token));

    axum::Server::bind(&"127.0.0.1:8283".parse().unwrap())
        .serve(app.into_make_service())
        .await?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum ReplspaceStatus {
    Ok,
    Err,
}

#[derive(Serialize)]
struct GithubTokenRes {
    status: ReplspaceStatus,
    token: Option<String>,
}

#[derive(Deserialize)]
struct GithubTokenReq {
    channel: i32,
}

async fn get_gh_token(_query: Option<Query<GithubTokenReq>>) -> (StatusCode, Json<GithubTokenRes>) {
    let session;
    if let Some(query) = _query {
        info!("Got git askpass for channel #{}", query.channel);

        let last_session = crate::LAST_SESSION_USING_CHANNEL.read().await;
        session = last_session.get(&query.channel).unwrap_or(&0).clone();
    } else {
        info!("Got git askpass without channel id");
        session = 0;
    }

    let nonce = TextNonce::new().into_string();
    let (tx, rx) = channel();

    let mut callback_table = crate::REPLSPACE_CALLBACKS.write().await;

    callback_table.insert(nonce.clone(), Some(tx));

    drop(callback_table);

    let to_send = JsMessage::Replspace(session, ReplspaceMessage::GithubTokenReq(nonce));

    let msg_lock = crate::CHANNEL_MESSAGES.read().await;

    for channel in msg_lock.values() {
        channel.push(to_send.clone());
    }
    // let queue = msg_lock.get(&cmd.channel).unwrap().clone();

    drop(msg_lock);

    let res;
    match rx.await {
        Ok(token) => res = token,
        Err(err) => {
            error!(
                "Got error awaiting replspace api github token fetcher callback {:#?}",
                err
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GithubTokenRes {
                    status: ReplspaceStatus::Err,
                    token: None,
                }),
            );
        }
    }

    let token;

    match res {
        ReplspaceMessage::GithubTokenRes(_token) => token = _token,
        _ => {
            error!(
                "Got unexpected result in replspace api github token fetcher {:#?}",
                res
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GithubTokenRes {
                    status: ReplspaceStatus::Err,
                    token: None,
                }),
            );
        }
    }

    (
        StatusCode::OK,
        Json(GithubTokenRes {
            status: ReplspaceStatus::Ok,
            token: Some(token),
        }),
    )
}

/* */
#[derive(Deserialize)]
struct OpenFileReq {
    filename: String,
    #[serde(rename = "waitForClose")]
    wait_for_close: bool,
    channel: Option<i32>,
}

#[derive(Serialize)]
struct OpenFileRes {
    pub status: ReplspaceStatus,
}

async fn open_file(Json(query): Json<OpenFileReq>) -> (StatusCode, Json<OpenFileRes>) {
    info!("Got git open file");
    let session;
    if let Some(channel) = query.channel {
        if channel != 0 {
            info!("Got git open file for channel #{}", channel);

            let last_session = crate::LAST_SESSION_USING_CHANNEL.read().await;
            session = last_session.get(&channel).unwrap_or(&0).clone();
        } else {
            info!("Got git open file with channel id set to 0 (unknown)");
            session = 0;
        }
    } else {
        info!("Got git open file without channel id");
        session = 0;
    }

    let nonce = TextNonce::new().into_string();
    let tx;
    let rx;
    if query.wait_for_close {
        let res = channel();
        tx = res.0;
        rx = Some(res.1);

        let mut callback_table = crate::REPLSPACE_CALLBACKS.write().await;

        callback_table.insert(nonce.clone(), Some(tx));

        drop(callback_table);
    } else {
        rx = None
    }

    let to_send = JsMessage::Replspace(
        session,
        ReplspaceMessage::OpenFileReq(query.filename, query.wait_for_close, nonce),
    );

    let msg_lock = crate::CHANNEL_MESSAGES.read().await;

    for channel in msg_lock.values() {
        channel.push(to_send.clone());
    }

    drop(msg_lock);

    if !query.wait_for_close {
        return (
            StatusCode::OK,
            Json(OpenFileRes {
                status: ReplspaceStatus::Ok,
            }),
        );
    }

    let res;
    match rx.expect("rx must be defined for this code to run").await {
        Ok(token) => res = token,
        Err(err) => {
            error!(
                "Got error awaiting replspace api open file fetcher callback {:#?}",
                err
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenFileRes {
                    status: ReplspaceStatus::Err,
                }),
            );
        }
    }

    match res {
        ReplspaceMessage::OpenFileRes => (
            StatusCode::OK,
            Json(OpenFileRes {
                status: ReplspaceStatus::Ok,
            }),
        ),
        _ => {
            error!(
                "Got unexpected result in replspace api github token fetcher {:#?}",
                res
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenFileRes {
                    status: ReplspaceStatus::Err,
                }),
            );
        }
    }
}

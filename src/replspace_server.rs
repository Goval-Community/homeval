use anyhow::Result;
use axum::{
    extract::Query,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use log::{as_debug, debug, error, info};
use serde::{Deserialize, Serialize};
use textnonce::TextNonce;
use tokio::sync::mpsc::channel;

use crate::{ChannelMessage, ReplspaceMessage};

pub async fn start_server() -> Result<()> {
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
        debug!(channel = query.channel; "Got git askpass");

        let last_session = crate::LAST_SESSION_USING_CHANNEL.read().await;
        session = *last_session.get(&query.channel).unwrap_or(&0);
    } else {
        debug!("Got git askpass without channel id");
        session = 0;
    }

    let nonce = TextNonce::new().into_string();
    let (tx, mut rx) = channel(1);

    let to_send =
        ChannelMessage::Replspace(session, ReplspaceMessage::GithubTokenReq(nonce), Some(tx));

    let msg_lock = crate::CHANNEL_MESSAGES.read().await;

    for channel in msg_lock.values() {
        channel.send(to_send.clone()).expect("TODO: deal with this");
    }

    drop(msg_lock);

    let msg = rx.recv().await;
    rx.close();
    let res = match msg {
        Some(token) => token,
        None => {
            error!("rx#recv() returned None in gh get token");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GithubTokenRes {
                    status: ReplspaceStatus::Err,
                    token: None,
                }),
            );
        }
    };

    let token = match res {
        ReplspaceMessage::GithubTokenRes(token) => token,
        _ => {
            error!(
                result = as_debug!(res);
                "Got unexpected result in replspace api github token fetcher"
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GithubTokenRes {
                    status: ReplspaceStatus::Err,
                    token: None,
                }),
            );
        }
    };

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
    debug!("Got git open file");
    let session;
    if let Some(channel) = query.channel {
        if channel != 0 {
            debug!(channel = channel; "Got git open file");

            let last_session = crate::LAST_SESSION_USING_CHANNEL.read().await;
            session = *last_session.get(&channel).unwrap_or(&0);
        } else {
            debug!("Got git open file with channel id set to 0 (unknown)");
            session = 0;
        }
    } else {
        debug!("Got git open file without channel id");
        session = 0;
    }

    let nonce = TextNonce::new().into_string();
    let tx;
    let _rx;
    if query.wait_for_close {
        let res = channel(1);
        tx = Some(res.0);
        _rx = Some(res.1);
    } else {
        _rx = None;
        tx = None;
    }

    let to_send = ChannelMessage::Replspace(
        session,
        ReplspaceMessage::OpenFileReq(query.filename, query.wait_for_close, nonce),
        tx,
    );

    let msg_lock = crate::CHANNEL_MESSAGES.read().await;

    for channel in msg_lock.values() {
        channel.send(to_send.clone()).expect("TODO: deal with this");
    }

    drop(msg_lock);

    let mut rx;
    if !query.wait_for_close {
        return (
            StatusCode::OK,
            Json(OpenFileRes {
                status: ReplspaceStatus::Ok,
            }),
        );
    } else {
        rx = _rx.expect("rx must be defined for this code to run");
    }

    let msg = rx.recv().await;
    rx.close();
    let res = match msg {
        Some(token) => token,
        None => {
            error!("rx#none() returned none in replspace api open file fetcher");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenFileRes {
                    status: ReplspaceStatus::Err,
                }),
            );
        }
    };

    match res {
        ReplspaceMessage::OpenFileRes => (
            StatusCode::OK,
            Json(OpenFileRes {
                status: ReplspaceStatus::Ok,
            }),
        ),
        _ => {
            error!(
                result = as_debug!(res);
                "Got unexpected result in replspace api github token fetcher"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenFileRes {
                    status: ReplspaceStatus::Err,
                }),
            )
        }
    }
}

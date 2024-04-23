use std::net::SocketAddr;

use anyhow::Result;
use axum::{
    extract::Query,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use homeval_services::{ChannelID, SessionID};
use serde::{Deserialize, Serialize};
use textnonce::TextNonce;
use tokio::sync::mpsc::channel;
use tracing::{debug, error, info};

use crate::{ChannelMessage, ReplspaceMessage};

pub async fn start_server() -> Result<()> {
    info!("Replspace api server listening on: 127.0.0.1:8283");
    let app = Router::new()
        .route("/files/open", post(open_file))
        .route("/github/token", get(get_gh_token));

    let listener = tokio::net::TcpListener::bind(&"127.0.0.1:8283".parse::<SocketAddr>()?)
        .await
        .unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
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

async fn get_gh_token(query: Option<Query<GithubTokenReq>>) -> (StatusCode, Json<GithubTokenRes>) {
    let session;
    if let Some(query) = query {
        debug!(channel = query.channel, "Got git askpass");

        let last_session = crate::LAST_SESSION_USING_CHANNEL.read().await;
        session = *last_session
            .get(&ChannelID(query.channel))
            .unwrap_or(&SessionID::zero());
    } else {
        debug!("Got git askpass without channel id");
        session = SessionID::zero();
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
    let Some(res) = msg else {
        error!("rx#recv() returned None in gh get token");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GithubTokenRes {
                status: ReplspaceStatus::Err,
                token: None,
            }),
        );
    };

    let ReplspaceMessage::GithubTokenRes(token) = res else {
        error!(
            result = ?res,
            "Got unexpected result in replspace api github token fetcher"
        );
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GithubTokenRes {
                status: ReplspaceStatus::Err,
                token: None,
            }),
        );
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
            debug!(channel, "Got git open file");

            let last_session = crate::LAST_SESSION_USING_CHANNEL.read().await;
            session = *last_session
                .get(&ChannelID(channel))
                .unwrap_or(&SessionID::zero());
        } else {
            debug!("Got git open file with channel id set to 0 (unknown)");
            session = SessionID::zero();
        }
    } else {
        debug!("Got git open file without channel id");
        session = SessionID::zero();
    }

    let nonce = TextNonce::new().into_string();
    let tx;
    let rx_option;
    if query.wait_for_close {
        let res = channel(1);
        tx = Some(res.0);
        rx_option = Some(res.1);
    } else {
        rx_option = None;
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
    if query.wait_for_close {
        rx = rx_option.expect("rx must be defined for this code to run");
    } else {
        return (
            StatusCode::OK,
            Json(OpenFileRes {
                status: ReplspaceStatus::Ok,
            }),
        );
    }

    let msg = rx.recv().await;
    rx.close();

    let Some(res) = msg else {
        error!("rx#none() returned none in replspace api open file fetcher");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(OpenFileRes {
                status: ReplspaceStatus::Err,
            }),
        );
    };

    if let ReplspaceMessage::OpenFileRes = res {
        (
            StatusCode::OK,
            Json(OpenFileRes {
                status: ReplspaceStatus::Ok,
            }),
        )
    } else {
        error!(
            result = ?res,
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

use std::sync::Arc;

use axum::{extract::Query, http::StatusCode, routing::get, Json, Router};
use deno_core::error::AnyError;
use log::{error, info};
use serde::{Deserialize, Serialize};
use textnonce::TextNonce;
use tokio::sync::oneshot::channel;

use crate::deno_extension::{messaging::ReplspaceMessage, JsMessage};

pub async fn start_server() -> Result<(), AnyError> {
    info!("Replspace api server listening on: 127.0.0.1:8283");
    let app = Router::new().route("/github/token", get(get_gh_token));

    // run it with hyper on 127.0.0.1:3000
    axum::Server::bind(&"127.0.0.1:8283".parse().unwrap())
        .serve(app.into_make_service())
        .await?;
    Ok(())
}

#[derive(Serialize)]
enum GHTokenStatus {
    Ok,
    Err,
}

#[derive(Serialize)]
struct GithubTokenRes {
    status: GHTokenStatus,
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
                    status: GHTokenStatus::Err,
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
                    status: GHTokenStatus::Err,
                    token: None,
                }),
            );
        }
    }

    (
        StatusCode::OK,
        Json(GithubTokenRes {
            status: GHTokenStatus::Ok,
            token: Some(token),
        }),
    )
}

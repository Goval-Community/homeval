pub struct Presence {
    users: Vec<goval::User>,
    files: HashMap<i32, goval::FileOpened>,
}
use crate::{ClientInfo, SendSessions};
use log::{as_debug, info, warn};
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use super::traits;
use anyhow::{format_err, Result};
use async_trait::async_trait;

impl Presence {
    pub fn new() -> Self {
        Self {
            users: vec![],
            files: HashMap::new(),
        }
    }
}

#[async_trait]
impl traits::Service for Presence {
    async fn open(&mut self, _info: &super::types::ChannelInfo) -> Result<()> {
        Ok(())
    }

    async fn message(
        &mut self,
        info: &super::types::ChannelInfo,
        message: goval::Command,
        session: i32,
    ) -> Result<Option<goval::Command>> {
        let body = match message.body.clone() {
            None => return Err(format_err!("Expected command body")),
            Some(body) => body,
        };

        match body {
            goval::command::Body::FollowUser(follow) => {
                let mut follow_notif = goval::Command::default();

                follow_notif.body = Some(goval::command::Body::FollowUser(goval::FollowUser {
                    session,
                }));

                info.send(follow_notif, SendSessions::Only(follow.session))
                    .await?;
                Ok(None)
            }
            goval::command::Body::UnfollowUser(unfollow) => {
                let mut unfollow_notif = goval::Command::default();

                unfollow_notif.body =
                    Some(goval::command::Body::UnfollowUser(goval::UnfollowUser {
                        session,
                    }));

                info.send(unfollow_notif, SendSessions::Only(unfollow.session))
                    .await?;
                Ok(None)
            }
            goval::command::Body::OpenFile(file) => {
                let user = info.sessions.get(&session).unwrap();

                let mut file_notif = goval::Command::default();

                let timestamp = Some(prost_types::Timestamp {
                    seconds: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                    nanos: 0,
                });

                let _inner = goval::FileOpened {
                    user_id: user.id,
                    file: file.file,
                    session,
                    timestamp,
                };

                file_notif.body = Some(goval::command::Body::FileOpened(_inner));

                info.send(file_notif, SendSessions::EveryoneExcept(session))
                    .await?;

                Ok(None)
            }
            _ => {
                warn!(cmd = as_debug!(message); "Unknown presence command");
                Ok(None)
            }
        }
    }

    async fn attach(
        &mut self,
        info: &super::types::ChannelInfo,
        client: ClientInfo,
        session: i32,
    ) -> Result<Option<goval::Command>> {
        let mut roster = goval::Command::default();
        let mut _inner = goval::Roster::default();

        let mut files = vec![];

        for file in self.files.values() {
            files.push(file.clone())
        }

        _inner.files = files;
        _inner.user = self.users.clone();
        roster.body = Some(goval::command::Body::Roster(_inner));

        let mut user = goval::User::default();
        user.session = session;
        user.id = client.id;
        user.name = client.username;

        let mut join = goval::Command::default();
        join.body = Some(goval::command::Body::Join(user.clone()));

        info.send(join, SendSessions::EveryoneExcept(session))
            .await?;

        self.users.push(user);

        Ok(Some(roster))
    }

    async fn detach(&mut self, info: &super::types::ChannelInfo, session: i32) -> Result<()> {
        self.files.remove(&session);
        let mut part = goval::Command::default();
        let mut flag = false;

        let users = self.users.clone();
        for (idx, user) in users.iter().enumerate() {
            if user.session == session {
                flag = true;
                part.body = Some(goval::command::Body::Part(user.clone()));
                self.users.swap_remove(idx);
                break;
            }
        }

        if !flag {
            return Err(format_err!(
                "Session {} missing from user list in Presence#detach",
                session
            ));
        }

        info!(e = as_debug!(part); "Presence#detach");
        info.send(part, SendSessions::EveryoneExcept(session))
            .await?;
        Ok(())
    }
}
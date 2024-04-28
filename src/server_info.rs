use std::sync::LazyLock;

use homeval_services::ServerInfo;

static SERVER_AUTHORS: LazyLock<Vec<&str>> =
    LazyLock::new(|| env!("CARGO_PKG_AUTHORS").split(':').collect());

pub static SERVER_INFO: ServerInfo = ServerInfo {
    name: env!("CARGO_PKG_NAME"),
    version: env!("CARGO_PKG_VERSION"),
    license: env!("CARGO_PKG_LICENSE"),
    repository: env!("CARGO_PKG_REPOSITORY"),
    description: env!("CARGO_PKG_DESCRIPTION"),
    start_time: &crate::START_TIME,
    authors: &SERVER_AUTHORS,
};

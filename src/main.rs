#![feature(lazy_cell)]

use std::sync::LazyLock;
use std::time::Instant;
use std::{collections::HashMap, io::Error, sync::Arc};

use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info};

use homeval_services::{
    config::dotreplit::DotReplit,
    messaging::ReplspaceMessage,
    ChannelMessage,
    ClientInfo,
    IPCMessage,
    ServiceMetadata,
    // ReplspaceMessage,
};

mod parse_paseto;

#[cfg(feature = "replspace")]
mod replspace_server;

#[cfg(feature = "repldb")]
mod repldb_server;

pub static START_TIME: LazyLock<Instant> = LazyLock::new(Instant::now);
static CPU_STATS: LazyLock<Arc<cpu_time::ProcessTime>> =
    LazyLock::new(|| Arc::new(cpu_time::ProcessTime::now()));

pub static IMPLEMENTED_SERVICES: LazyLock<Vec<String>> = LazyLock::new(Vec::new);

pub static DOTREPLIT_CONFIG: LazyLock<Arc<RwLock<DotReplit>>> = LazyLock::new(|| {
    Arc::new(RwLock::const_new(
        toml::from_str(&std::fs::read_to_string(".replit").unwrap_or("".to_string())).unwrap(),
    ))
});

static MAX_SESSION: LazyLock<Mutex<i32>> = LazyLock::new(|| Mutex::new(0));

static SESSION_CHANNELS: LazyLock<RwLock<HashMap<i32, Vec<i32>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static SESSION_CLIENT_INFO: LazyLock<RwLock<HashMap<i32, ClientInfo>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static CHANNEL_MESSAGES: LazyLock<
    RwLock<HashMap<i32, tokio::sync::mpsc::UnboundedSender<ChannelMessage>>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));
static CHANNEL_METADATA: LazyLock<RwLock<HashMap<i32, ServiceMetadata>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static CHANNEL_SESSIONS: LazyLock<RwLock<HashMap<i32, Vec<i32>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static SESSION_MAP: LazyLock<RwLock<HashMap<i32, mpsc::UnboundedSender<IPCMessage>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

// pty and cmd's
static PROCCESS_WRITE_MESSAGES: LazyLock<
    RwLock<HashMap<u32, Arc<deadqueue::unlimited::Queue<String>>>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));
static PROCCESS_CHANNEL_TO_ID: LazyLock<RwLock<HashMap<i32, u32>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

// Hashmap for channel id -> last session that sent it a message
static LAST_SESSION_USING_CHANNEL: LazyLock<RwLock<HashMap<i32, i32>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

// static REPLSPACE_CALLBACKS: LazyLock<
//     RwLock<HashMap<String, Option<tokio::sync::oneshot::Sender<ReplspaceMessage>>>>,
// > = LazyLock::new(|| RwLock::new(HashMap::new()));

static CHILD_PROCS_ENV_BASE: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

#[cfg(feature = "database")]
mod database;
#[cfg(feature = "database")]
pub use database::DATABASE;

mod goval_server;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();
    debug!("Initializing lazy statics");
    LazyLock::force(&START_TIME);
    LazyLock::force(&CPU_STATS);

    debug!("Lazy statics initialized successfully");

    std::env::set_var("HOMEVAL_START_DIR", std::env::current_dir()?);

    // console_subscriber::init();

    #[cfg(feature = "database")]
    database::setup().await.unwrap();

    info!("Starting homeval!");

    #[cfg(feature = "replspace")]
    tokio::spawn(replspace_server::start_server());

    #[cfg(feature = "repldb")]
    tokio::spawn(repldb_server::start_server());

    goval_server::start_server().await.unwrap();

    Ok(())
}

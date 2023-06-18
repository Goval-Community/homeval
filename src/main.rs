use deno_extension::messaging::ReplspaceMessage;
use homeval::goval;

use std::time::Instant;
use std::{collections::HashMap, io::Error, sync::Arc};

use log::{debug, info};
use tokio::sync::{mpsc, Mutex, RwLock};

mod channels;
use channels::IPCMessage;

mod deno_extension;
use deno_extension::{JsMessage, Service};

mod parse_paseto;
use parse_paseto::ClientInfo;

#[cfg(debug_assertions)]
mod services_debug;
#[cfg(debug_assertions)]
use services_debug as services;

#[cfg(not(debug_assertions))]
mod services_release;
#[cfg(not(debug_assertions))]
use services_release as services;

mod config;
use config::dotreplit::DotReplit;

#[cfg(feature = "replspace")]
mod replspace_server;

#[cfg(feature = "repldb")]
mod repldb_server;

use lazy_static::lazy_static;
lazy_static! {
    pub static ref START_TIME: Instant = Instant::now();
    pub static ref IMPLEMENTED_SERVICES: Vec<String> = {
        let mut services = vec![];
        for file in services::get_all().expect("Error when looping over `services/`") {
            if let Some(extension) = file.extension() {
                if extension == "js" {
                    let mut service_path = file
                        .file_name()
                        .expect("File name missing while extension exists???")
                        .to_str()
                        .expect("failed to convert path OsString to String")
                        .to_string();

                    service_path
                        .pop()
                        .expect("Impossible case when removing .js extension from service file");
                    service_path
                        .pop()
                        .expect("Impossible case when removing .js extension from service file");
                    service_path
                        .pop()
                        .expect("Impossible case when removing .js extension from service file");

                    services.push(service_path);
                }
            }
        }
        services
    };
    pub static ref DOTREPLIT_CONFIG: DotReplit =
        toml::from_str(&std::fs::read_to_string(".replit").unwrap_or("".to_string())).unwrap();
}

lazy_static! {
    static ref MAX_SESSION: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
    static ref MAX_CHANNEL: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

lazy_static! {
    static ref SESSION_CHANNELS: RwLock<HashMap<i32, Vec<i32>>> = RwLock::new(HashMap::new());
    static ref SESSION_CLIENT_INFO: RwLock<HashMap<i32, ClientInfo>> = RwLock::new(HashMap::new());
    static ref CHANNEL_MESSAGES: Arc<RwLock<HashMap<i32, Arc<deadqueue::unlimited::Queue<JsMessage>>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    static ref CHANNEL_METADATA: RwLock<HashMap<i32, Service>> = RwLock::new(HashMap::new());
    static ref CHANNEL_SESSIONS: RwLock<HashMap<i32, Vec<i32>>> = RwLock::new(HashMap::new());

    static ref SESSION_MAP: Arc<RwLock<HashMap<i32, mpsc::UnboundedSender<IPCMessage>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // pty and cmd's
    static ref PROCCESS_WRITE_MESSAGES: RwLock<HashMap<u32, Arc<deadqueue::unlimited::Queue<String>>>> =
    RwLock::new(HashMap::new());
    static ref PROCCESS_CHANNEL_TO_ID: RwLock<HashMap<i32, u32>> = RwLock::new(HashMap::new());

    static ref CPU_STATS: Arc<cpu_time::ProcessTime> = Arc::new(cpu_time::ProcessTime::now());

    // Hashmap for channel id -> last session that sent it a message
    static ref LAST_SESSION_USING_CHANNEL: Arc<RwLock<HashMap<i32, i32>>> = Arc::new(RwLock::new(HashMap::new()));

    static ref REPLSPACE_CALLBACKS: Arc<RwLock<HashMap<String, Option<tokio::sync::oneshot::Sender<ReplspaceMessage>>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    static ref CHILD_PROCS_ENV_BASE: Arc<RwLock<HashMap<String, String>>> = Arc::new(RwLock::new(HashMap::new()));
}

#[cfg(feature = "database")]
mod database;
#[cfg(feature = "database")]
pub use database::DATABASE;

mod goval_server;

#[tokio::main]
async fn main() -> Result<(), Error> {
    debug!("Initializing lazy statics");
    lazy_static::initialize(&START_TIME);
    lazy_static::initialize(&IMPLEMENTED_SERVICES);
    lazy_static::initialize(&DOTREPLIT_CONFIG);
    lazy_static::initialize(&MAX_SESSION);
    lazy_static::initialize(&MAX_CHANNEL);
    lazy_static::initialize(&SESSION_CHANNELS);
    lazy_static::initialize(&SESSION_CLIENT_INFO);
    lazy_static::initialize(&CHANNEL_MESSAGES);
    lazy_static::initialize(&CHANNEL_METADATA);
    // lazy_static::initialize(&CHANNEL_SESSIONS);
    lazy_static::initialize(&SESSION_MAP);
    lazy_static::initialize(&PROCCESS_WRITE_MESSAGES);
    lazy_static::initialize(&PROCCESS_CHANNEL_TO_ID);
    lazy_static::initialize(&CPU_STATS);
    lazy_static::initialize(&LAST_SESSION_USING_CHANNEL);
    lazy_static::initialize(&REPLSPACE_CALLBACKS);
    lazy_static::initialize(&CHILD_PROCS_ENV_BASE);
    debug!("Lazy statics initialized successfully");

    // console_subscriber::init();
    let _ = env_logger::try_init().unwrap();

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

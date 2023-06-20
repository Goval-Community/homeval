use deno_core::{op, OpDecl};
use log::{as_debug, debug, error, trace};
use notify_debouncer_full::{
    new_debouncer,
    notify::{self, event::ModifyKind, Event, EventKind, RecommendedWatcher, Watcher},
    DebounceEventResult, Debouncer, FileIdMap,
};
use serde::Serialize;

use deno_core::error::AnyError;

use std::{
    collections::HashMap,
    io::Error,
    path::Path,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::sync::{Mutex, RwLock};

static FILE_WATCHER_MAP: LazyLock<
    RwLock<HashMap<u32, Arc<Mutex<Debouncer<RecommendedWatcher, FileIdMap>>>>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));
static FILE_WATCHER_MESSAGES: LazyLock<
    RwLock<HashMap<u32, Arc<deadqueue::unlimited::Queue<FinalEvent>>>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));
static MAX_WATCHER: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(0));

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FinalEvent {
    Remove(String),
    Create(String),
    Modify(String),
    Rename(String, String),
    Err(String),
    Shutdown(bool),
}

fn notify_event_to_final(event: &Event) -> Result<Option<FinalEvent>, AnyError> {
    let base = std::env::current_dir()?;
    let file_name = event.paths[0]
        .strip_prefix(base.clone())?
        .to_str()
        .unwrap()
        .to_string();
    match event.kind {
        EventKind::Create(_) => Ok(Some(FinalEvent::Create(file_name))),
        EventKind::Modify(_kind @ ModifyKind::Name(notify::event::RenameMode::Both)) => {
            Ok(Some(FinalEvent::Rename(
                file_name,
                event.paths[1]
                    .strip_prefix(base)?
                    .to_str()
                    .unwrap()
                    .to_string(),
            )))
        }
        EventKind::Modify(_kind @ ModifyKind::Name(notify::event::RenameMode::From)) => {
            Ok(Some(FinalEvent::Remove(file_name.to_string())))
        }
        EventKind::Modify(_kind @ ModifyKind::Name(notify::event::RenameMode::To)) => {
            Ok(Some(FinalEvent::Create(file_name.to_string())))
        }
        EventKind::Modify(_) => Ok(Some(FinalEvent::Modify(file_name))),
        EventKind::Remove(_) => Ok(Some(FinalEvent::Remove(file_name))),
        _ => Ok(None),
    }
}

#[op]
async fn op_shutdown_filewatcher(watcher: u32) -> Result<(), AnyError> {
    let mut _read = FILE_WATCHER_MAP.write().await;
    if !_read.contains_key(&watcher) {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::NotFound,
            "Watcher not found",
        )));
    }

    let debouncer_lock = _read.get(&watcher).unwrap().clone();
    _read.remove(&watcher);
    drop(_read);

    match Arc::try_unwrap(debouncer_lock) {
        Ok(inner) => {
            inner.into_inner().stop_nonblocking();
        }
        Err(_) => {
            debug!(watcher = watcher; "Could not steal Debouncer from Arc<Debouncer>");
        }
    }

    let mut _read = FILE_WATCHER_MESSAGES.write().await;
    if !_read.contains_key(&watcher) {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::NotFound,
            "Watcher not found",
        )));
    }

    let queue = _read.get(&watcher).unwrap().clone();
    queue.push(FinalEvent::Shutdown(true));
    _read.remove(&watcher);
    drop(_read);

    trace!(watcher = watcher; "Shutdown watcher");

    Ok(())
}

#[op]
async fn op_make_filewatcher() -> Result<u32, AnyError> {
    let mut max_watcher = MAX_WATCHER.lock().await;
    *max_watcher += 1;
    let watcher_id = max_watcher.clone();
    drop(max_watcher);

    let queue: Arc<deadqueue::unlimited::Queue<FinalEvent>> =
        Arc::new(deadqueue::unlimited::Queue::new());

    FILE_WATCHER_MESSAGES
        .write()
        .await
        .insert(watcher_id, queue.clone());

    let queue_writer = queue.clone();
    // tokio::spawn(async move {
    let queue_debounced = queue_writer.clone();
    tokio::task::spawn_blocking(move || {
        let debouncer: Debouncer<RecommendedWatcher, notify_debouncer_full::FileIdMap> =
            new_debouncer(
                Duration::from_secs(1),
                None,
                move |result: DebounceEventResult| match result {
                    Ok(events) => events.iter().for_each(|event| {
                        if let Some(final_event) = notify_event_to_final(event).unwrap() {
                            queue_debounced.push(final_event);
                        }
                    }),
                    Err(errors) => errors.iter().for_each(|error| {
                        error!(error = as_debug!(error); "Error in debouncer");
                        queue_debounced.push(FinalEvent::Err(error.to_string()))
                    }),
                },
            )
            .unwrap();

        let mut watcher_map = FILE_WATCHER_MAP.blocking_write();
        watcher_map.insert(watcher_id, Arc::new(Mutex::new(debouncer)));
    })
    .await?;
    // });
    Ok(watcher_id)
}

#[op]
async fn op_recv_fsevent(watcher: u32) -> Result<FinalEvent, AnyError> {
    let _read = FILE_WATCHER_MESSAGES.read().await;
    if !_read.contains_key(&watcher) {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::NotFound,
            "Watcher not found",
        )));
    }

    let queue = _read.get(&watcher).unwrap().clone();
    drop(_read);

    let res = queue.pop().await;
    Ok(res)
}

#[op]
async fn op_watch_files(watcher: u32, files: Vec<String>) -> Result<(), AnyError> {
    let _read = FILE_WATCHER_MAP.read().await;
    if !_read.contains_key(&watcher) {
        return Err(AnyError::new(Error::new(
            std::io::ErrorKind::NotFound,
            "Watcher not found",
        )));
    }

    let debouncer_lock = _read.get(&watcher).unwrap().clone();
    drop(_read);

    let mut debouncer = debouncer_lock.lock().await;

    for file in files {
        let path = Path::new(&file);
        debouncer
            .watcher()
            .watch(path, notify::RecursiveMode::NonRecursive)?;
        debouncer
            .cache()
            .add_root(path, notify::RecursiveMode::NonRecursive)
    }

    Ok(())
}

pub fn get_op_decls() -> Vec<OpDecl> {
    vec![
        op_shutdown_filewatcher::decl(),
        op_make_filewatcher::decl(),
        op_recv_fsevent::decl(),
        op_watch_files::decl(),
    ]
}

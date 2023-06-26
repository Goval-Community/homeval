use log::{as_debug, error};
use notify_debouncer_full::{
    new_debouncer,
    notify::{self, event::ModifyKind, Event, EventKind, RecommendedWatcher, Watcher},
    DebounceEventResult, Debouncer,
};
use serde::Serialize;

use anyhow::Result;

use std::{path::Path, time::Duration};
use tokio::sync::broadcast;

// static FILE_WATCHER_MAP: LazyLock<
//     RwLock<HashMap<u32, Arc<Mutex<Debouncer<RecommendedWatcher, FileIdMap>>>>>,
// > = LazyLock::new(|| RwLock::new(HashMap::new()));
// static FILE_WATCHER_MESSAGES: LazyLock<
//     RwLock<HashMap<u32, Arc<deadqueue::unlimited::Queue<FSEvent>>>>,
// > = LazyLock::new(|| RwLock::new(HashMap::new()));
// static MAX_WATCHER: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(0));

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FSEvent {
    Remove(String),
    Create(String),
    Modify(String),
    Rename(String, String),
    Err(String),
}

pub struct FSWatcher {
    debouncer: Debouncer<RecommendedWatcher, notify_debouncer_full::FileIdMap>,
    writer: broadcast::Sender<FSEvent>,
    reader: broadcast::Receiver<FSEvent>,
}

impl FSWatcher {
    pub async fn new() -> Result<FSWatcher> {
        let (writer, reader) = broadcast::channel::<FSEvent>(5);

        // FILE_WATCHER_MESSAGES
        //     .write()
        //     .await
        //     .insert(watcher_id, queue.clone());

        // tokio::spawn(async move {
        let debounce_writer = writer.clone();
        let debouncer = tokio::task::spawn_blocking(move || {
            new_debouncer(
                Duration::from_secs(1),
                None,
                move |result: DebounceEventResult| match result {
                    Ok(events) => events.iter().for_each(|event| {
                        if let Some(final_event) = notify_event_to_final(event).unwrap() {
                            debounce_writer
                                .send(final_event)
                                .expect("TODO: handle this");
                        }
                    }),
                    Err(errors) => errors.iter().for_each(|error| {
                        error!(error = as_debug!(error); "Error in debouncer");
                        debounce_writer
                            .send(FSEvent::Err(error.to_string()))
                            .expect("TODO: handle this");
                    }),
                },
            )

            // let mut watcher_map = FILE_WATCHER_MAP.blocking_write();
            // watcher_map.insert(watcher_id, Arc::new(Mutex::new(debouncer)));
        })
        .await??;
        // });

        Ok(FSWatcher {
            debouncer,
            reader,
            writer,
        })
    }

    pub async fn watch(&mut self, files: Vec<String>) -> Result<()> {
        for file in files {
            let path = Path::new(&file);
            self.debouncer
                .watcher()
                .watch(path, notify::RecursiveMode::NonRecursive)?;
            self.debouncer
                .cache()
                .add_root(path, notify::RecursiveMode::NonRecursive)
        }

        Ok(())
    }

    pub async fn shutdown(self) {
        self.debouncer.stop_nonblocking();
        drop(self.writer)
    }

    pub async fn get_event_reader(&mut self) -> broadcast::Receiver<FSEvent> {
        self.reader.resubscribe()
    }
}

fn notify_event_to_final(event: &Event) -> Result<Option<FSEvent>> {
    let base = std::env::current_dir()?;
    let file_name = event.paths[0]
        .strip_prefix(base.clone())?
        .to_str()
        .unwrap()
        .to_string();
    match event.kind {
        EventKind::Create(_) => Ok(Some(FSEvent::Create(file_name))),
        EventKind::Modify(_kind @ ModifyKind::Name(notify::event::RenameMode::Both)) => {
            Ok(Some(FSEvent::Rename(
                file_name,
                event.paths[1]
                    .strip_prefix(base)?
                    .to_str()
                    .unwrap()
                    .to_string(),
            )))
        }
        EventKind::Modify(_kind @ ModifyKind::Name(notify::event::RenameMode::From)) => {
            Ok(Some(FSEvent::Remove(file_name.to_string())))
        }
        EventKind::Modify(_kind @ ModifyKind::Name(notify::event::RenameMode::To)) => {
            Ok(Some(FSEvent::Create(file_name.to_string())))
        }
        EventKind::Modify(_) => Ok(Some(FSEvent::Modify(file_name))),
        EventKind::Remove(_) => Ok(Some(FSEvent::Remove(file_name))),
        _ => Ok(None),
    }
}

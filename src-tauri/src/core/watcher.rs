//! Debounced file watching. `notify-debouncer-full` with a `FileIdMap`, not
//! `RecommendedCache`: the id map is what lets a rename arrive as one event
//! instead of a delete plus a create.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{EventKind, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;

use crate::error::Result;

/// Long enough that a DAW writing a file in several passes, or a copy landing
/// in chunks, produces one event rather than a dozen half-written ones.
const DEBOUNCE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub enum WatchEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

type Watched = Debouncer<notify::RecommendedWatcher, FileIdMap>;

pub struct FileWatcher {
    tx: UnboundedSender<WatchEvent>,
    /// One debouncer per root rather than one shared watcher: unwatching a root
    /// then has nothing to unpick, which is the operation that goes wrong when
    /// a user removes a folder mid-scan.
    debouncers: Mutex<HashMap<PathBuf, Watched>>,
    watched: Mutex<Vec<PathBuf>>,
}

impl FileWatcher {
    pub fn new(tx: UnboundedSender<WatchEvent>) -> Result<Self> {
        Ok(Self {
            tx,
            debouncers: Mutex::new(HashMap::new()),
            watched: Mutex::new(Vec::new()),
        })
    }

    pub fn watch(&self, path: &Path) -> Result<()> {
        if self.debouncers.lock().contains_key(path) {
            return Ok(());
        }
        let tx = self.tx.clone();
        let mut debouncer = new_debouncer(
            DEBOUNCE,
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    for event in events {
                        for mapped in translate(&event.event) {
                            let _ = tx.send(mapped);
                        }
                    }
                }
                // A watch error is not fatal: the reconciler on the next launch
                // catches whatever the watcher missed, which is why it exists.
                Err(errors) => {
                    for e in errors {
                        tracing::warn!("file watcher error: {}", e);
                    }
                }
            },
        )?;
        debouncer.watcher().watch(path, RecursiveMode::Recursive)?;
        debouncer
            .cache()
            .add_root(path, RecursiveMode::Recursive);

        self.debouncers.lock().insert(path.to_path_buf(), debouncer);
        let mut watched = self.watched.lock();
        if !watched.iter().any(|p| p == path) {
            watched.push(path.to_path_buf());
        }
        Ok(())
    }

    pub fn unwatch(&self, path: &Path) -> Result<()> {
        // Dropping the debouncer stops its thread and releases the watch; there
        // is no partial state left behind for a later re-add to trip over.
        self.debouncers.lock().remove(path);
        self.watched.lock().retain(|p| p != path);
        Ok(())
    }

    pub fn watched_paths(&self) -> Vec<PathBuf> {
        self.watched.lock().clone()
    }
}

/// One notify event to zero or more [`WatchEvent`]s.
///
/// A rename arrives as a single `Both` event precisely because the debouncer
/// carries a `FileIdMap`; without it this is a delete followed by a create, and
/// the row loses its rating, cues and analysis on every file move.
fn translate(event: &notify::Event) -> Vec<WatchEvent> {
    use notify::event::{CreateKind, ModifyKind, RenameMode};
    match event.kind {
        EventKind::Create(CreateKind::Any | CreateKind::File | CreateKind::Other) => {
            event.paths.iter().cloned().map(WatchEvent::Created).collect()
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() == 2 => {
            vec![WatchEvent::Renamed {
                from: event.paths[0].clone(),
                to: event.paths[1].clone(),
            }]
        }
        // A one-sided rename is all the platform gave us. Treating `From` as a
        // delete and `To` as a create is lossy but correct; the reconciler and
        // the content-hash cache put the analysis back.
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            event.paths.iter().cloned().map(WatchEvent::Deleted).collect()
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            event.paths.iter().cloned().map(WatchEvent::Created).collect()
        }
        EventKind::Modify(_) => event
            .paths
            .iter()
            .cloned()
            .map(WatchEvent::Modified)
            .collect(),
        EventKind::Remove(_) => event.paths.iter().cloned().map(WatchEvent::Deleted).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RenameMode};

    fn event(kind: EventKind, paths: &[&str]) -> notify::Event {
        notify::Event {
            kind,
            paths: paths.iter().map(PathBuf::from).collect(),
            attrs: Default::default(),
        }
    }

    #[test]
    fn a_rename_arrives_as_one_event_and_keeps_both_paths() {
        // The FileIdMap is what makes this possible. As a delete plus a create,
        // every file move would cost the row its rating, cues and analysis.
        let mapped = translate(&event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &["/m/old.mp3", "/m/new.mp3"],
        ));
        assert_eq!(mapped.len(), 1);
        match &mapped[0] {
            WatchEvent::Renamed { from, to } => {
                assert!(from.ends_with("old.mp3"));
                assert!(to.ends_with("new.mp3"));
            }
            other => panic!("expected a rename, got {other:?}"),
        }
    }

    #[test]
    fn creates_modifies_and_removes_map_to_their_own_events() {
        assert!(matches!(
            translate(&event(EventKind::Create(CreateKind::File), &["/m/a.mp3"]))[0],
            WatchEvent::Created(_)
        ));
        assert!(matches!(
            translate(&event(
                EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Any)),
                &["/m/a.mp3"]
            ))[0],
            WatchEvent::Modified(_)
        ));
        assert!(matches!(
            translate(&event(
                EventKind::Remove(notify::event::RemoveKind::File),
                &["/m/a.mp3"]
            ))[0],
            WatchEvent::Deleted(_)
        ));
    }

    #[test]
    fn an_event_we_do_not_model_is_dropped_rather_than_guessed_at() {
        assert!(translate(&event(EventKind::Access(notify::event::AccessKind::Read), &["/m/a.mp3"])).is_empty());
        assert!(translate(&event(EventKind::Other, &["/m/a.mp3"])).is_empty());
    }

    #[test]
    fn watching_a_root_twice_is_idempotent_and_unwatching_clears_it() {
        let dir = tempfile::TempDir::new().unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let watcher = FileWatcher::new(tx).unwrap();

        watcher.watch(dir.path()).unwrap();
        watcher.watch(dir.path()).unwrap();
        assert_eq!(watcher.watched_paths().len(), 1);

        watcher.unwatch(dir.path()).unwrap();
        assert!(watcher.watched_paths().is_empty());
    }
}

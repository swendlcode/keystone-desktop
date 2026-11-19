//! One-time repair for paths stored with `.` / `..` / doubled separators.
//!
//! See `core::paths` for why such a path is unreachable rather than untidy.
//! This pass exists because the importer wrote fifty of them before the
//! normalisation in `music_library` was added, and nothing else can heal them:
//! the reconciler only visits rows that already fall under a watched root, and
//! these do not.
//!
//! It runs before the reconciler at launch, so the rows it repairs are
//! reconciled in the same pass and come back as `indexed` with their analysis,
//! rating and cues intact.

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::paths;
use crate::db::TrackRepository;
use crate::error::Result;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PathRepairReport {
    /// Rows whose stored path was rewritten.
    pub repaired: usize,
    /// Rows dropped because the normalised path was already in the table.
    pub deduped: usize,
}

impl PathRepairReport {
    pub fn is_empty(&self) -> bool {
        self.repaired == 0 && self.deduped == 0
    }
}

/// Normalises every stored path that needs it.
///
/// Where normalising would collide with a row that already holds the clean
/// path, the unnormalised row is the one that goes: the clean one is what the
/// scanner maintains and what the watched root can see, so keeping both would
/// leave a permanent duplicate that only one of the two halves of the app can
/// find. That is the only case here that loses anything, and what it loses is
/// a row no view could reach.
pub fn run(tracks: &Arc<TrackRepository>) -> Result<PathRepairReport> {
    let mut report = PathRepairReport::default();

    for (id, path) in tracks.unnormalized_paths()? {
        let normalized = paths::normalize(Path::new(&path));
        let normalized = normalized.to_string_lossy();
        if normalized == path {
            continue;
        }

        match tracks.get_by_path(&normalized)? {
            Some(existing) if existing.id != id => {
                tracks.delete(&id)?;
                report.deduped += 1;
                tracing::info!("dropped duplicate of {} stored as {}", normalized, path);
            }
            _ => {
                tracks.renormalize_path(&id, &normalized)?;
                report.repaired += 1;
            }
        }
    }

    if !report.is_empty() {
        tracing::info!(
            "path repair: {} normalised, {} duplicates dropped",
            report.repaired,
            report.deduped
        );
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::Harness;

    #[test]
    fn a_playlist_relative_path_is_rewritten_in_place() {
        let h = Harness::new();
        let id = h.add("/crate/Playlists/../Songs/Cola.mp3");

        let report = run(&h.tracks).unwrap();
        assert_eq!(report, PathRepairReport { repaired: 1, deduped: 0 });

        // Same row, same id: everything hanging off the id survives.
        let track = h.tracks.get_by_id(&id).unwrap().unwrap();
        assert_eq!(track.path, "/crate/Songs/Cola.mp3");
    }

    /// The point of the whole exercise: the row becomes visible to the root
    /// that owns it.
    #[test]
    fn the_repaired_row_is_reachable_from_its_watched_root() {
        let h = Harness::new();
        h.add("/crate/Playlists/../Songs/Cola.mp3");
        assert!(h
            .tracks
            .existing_paths("/crate/Songs/", 0, 50)
            .unwrap()
            .is_empty());

        run(&h.tracks).unwrap();
        assert_eq!(
            h.tracks.existing_paths("/crate/Songs/", 0, 50).unwrap(),
            vec!["/crate/Songs/Cola.mp3".to_string()]
        );
    }

    /// A second launch finds nothing to do.
    #[test]
    fn the_pass_is_idempotent() {
        let h = Harness::new();
        h.add("/crate/Playlists/../Songs/Cola.mp3");
        run(&h.tracks).unwrap();
        assert!(run(&h.tracks).unwrap().is_empty());
    }

    #[test]
    fn a_clean_library_is_left_alone() {
        let h = Harness::new();
        h.add("/crate/Songs/Cola.mp3");
        h.add("/crate/Songs/Fred again.. - Delilah.mp3");
        assert!(run(&h.tracks).unwrap().is_empty());
    }

    /// Normalising onto a path the scanner already indexed drops the
    /// unreachable row rather than failing on `UNIQUE(path)`.
    #[test]
    fn a_collision_with_the_clean_row_drops_the_unreachable_one() {
        let h = Harness::new();
        let clean = h.add("/crate/Songs/Cola.mp3");
        let dupe = h.add("/crate/Playlists/../Songs/Cola.mp3");

        let report = run(&h.tracks).unwrap();
        assert_eq!(report, PathRepairReport { repaired: 0, deduped: 1 });
        assert!(h.tracks.get_by_id(&dupe).unwrap().is_none());
        assert_eq!(
            h.tracks.get_by_id(&clean).unwrap().unwrap().path,
            "/crate/Songs/Cola.mp3"
        );
    }
}

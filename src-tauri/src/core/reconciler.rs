//! Startup diff: touch what is still there, mark what is gone.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::db::{LibraryRepository, TrackRepository};
use crate::error::Result;
use crate::models::ReconcileReport;

/// Paths per round trip. Big enough that a 20,000-track root is forty
/// statements rather than twenty thousand, small enough that nothing here ever
/// holds a whole library in memory.
const CHUNK: usize = 500;

pub struct Reconciler {
    tracks: Arc<TrackRepository>,
    library: Arc<LibraryRepository>,
}

impl Reconciler {
    pub fn new(tracks: Arc<TrackRepository>, library: Arc<LibraryRepository>) -> Self {
        Self { tracks, library }
    }

    /// Reads the existing paths in chunks and touches `last_seen_at` in chunks.
    /// Loading 20,000 paths into one Vec and one IN list is the version of this
    /// that stalls the UI on launch.
    ///
    /// Marks missing rather than deleting: an unmounted external drive is the
    /// common case, and a DJ library on one is normal. Deleting is behind the
    /// explicit Clean cache action.
    pub fn run(&self, roots: &[String]) -> Result<ReconcileReport> {
        let clock = Instant::now();
        let mut report = ReconcileReport::default();

        for root in roots {
            // An unreachable root is one UPDATE rather than ten thousand
            // `exists()` calls against a drive that is not plugged in.
            if !Path::new(root).is_dir() {
                report.marked_missing += self.library.mark_missing_under(root)?;
                continue;
            }
            let (seen, touched, marked) = self.visit(root)?;
            report.seen += seen;
            report.touched += touched;
            report.marked_missing += marked;
        }
        report.elapsed_ms = clock.elapsed().as_millis() as u64;
        Ok(report)
    }

    /// Stats every known path under `root`, touching the ones still there and
    /// marking the ones that are not. Returns
    /// `(rows examined, rows touched, rows marked missing)`.
    fn visit(&self, root: &str) -> Result<(usize, usize, usize)> {
        let mut offset = 0usize;
        let mut seen = 0usize;
        let mut touched = 0usize;
        let mut marked = 0usize;
        loop {
            let paths = self.tracks.existing_paths(root, offset, CHUNK)?;
            if paths.is_empty() {
                break;
            }
            seen += paths.len();
            // Paging by offset is stable here because the ordering is on
            // `path`, and nothing in this loop writes a path.
            offset += paths.len();

            let (present, gone): (Vec<String>, Vec<String>) =
                paths.into_iter().partition(|p| Path::new(p).is_file());
            if !present.is_empty() {
                touched += self.tracks.touch_last_seen_by_paths(&present)?;
            }
            if !gone.is_empty() {
                marked += self.tracks.mark_missing_by_paths(&gone)?;
            }
        }
        Ok((seen, touched, marked))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::Harness;
    use crate::models::IndexStatus;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"audio").unwrap();
    }

    fn reconciler(h: &Harness) -> Reconciler {
        Reconciler::new(h.tracks.clone(), h.library.clone())
    }

    #[test]
    fn a_file_that_is_gone_is_marked_missing_and_not_deleted() {
        let h = Harness::new();
        let dir = tempfile::TempDir::new().unwrap();
        let root = format!("{}/", dir.path().to_string_lossy());

        touch(&dir.path().join("here.mp3"));
        touch(&dir.path().join("gone.mp3"));
        let here = h.add(&format!("{root}here.mp3"));
        let gone = h.add(&format!("{root}gone.mp3"));
        std::fs::remove_file(dir.path().join("gone.mp3")).unwrap();

        let report = reconciler(&h).run(&[root]).unwrap();
        assert_eq!(report.seen, 2);
        assert_eq!(report.marked_missing, 1);

        assert_eq!(
            h.tracks.get_by_id(&gone).unwrap().unwrap().index_status,
            IndexStatus::Missing
        );
        assert_eq!(
            h.tracks.get_by_id(&here).unwrap().unwrap().index_status,
            IndexStatus::Indexed
        );
        // The row survives: the user's rating, cues and analysis are still
        // there if the file comes back.
        assert!(h.tracks.get_by_id(&gone).unwrap().is_some());
    }

    #[test]
    fn a_file_that_comes_back_is_restored_rather_than_staying_missing() {
        let h = Harness::new();
        let dir = tempfile::TempDir::new().unwrap();
        let root = format!("{}/", dir.path().to_string_lossy());
        let path = dir.path().join("back.mp3");
        touch(&path);
        let id = h.add(&format!("{root}back.mp3"));

        std::fs::remove_file(&path).unwrap();
        reconciler(&h).run(&[root.clone()]).unwrap();
        assert_eq!(
            h.tracks.get_by_id(&id).unwrap().unwrap().index_status,
            IndexStatus::Missing
        );

        // Re-plugging the drive must heal itself; a user should not have to
        // find a re-scan button to undo something they did not do.
        touch(&path);
        let report = reconciler(&h).run(&[root]).unwrap();
        assert_eq!(report.marked_missing, 0);
        assert_eq!(
            h.tracks.get_by_id(&id).unwrap().unwrap().index_status,
            IndexStatus::Indexed
        );
    }

    #[test]
    fn an_unreachable_root_marks_its_subtree_without_statting_every_file() {
        let h = Harness::new();
        let ids: Vec<String> = (0..3)
            .map(|i| h.add(&format!("/Volumes/USB/{i}.mp3")))
            .collect();
        let kept = h.add("/Users/dj/Music/local.mp3");

        let report = reconciler(&h).run(&["/Volumes/USB/".into()]).unwrap();
        assert_eq!(report.marked_missing, 3);
        assert_eq!(report.seen, 0, "an absent root is not walked at all");
        for id in &ids {
            assert_eq!(
                h.tracks.get_by_id(id).unwrap().unwrap().index_status,
                IndexStatus::Missing
            );
        }
        // A root that was not in the list is left entirely alone.
        assert_eq!(
            h.tracks.get_by_id(&kept).unwrap().unwrap().index_status,
            IndexStatus::Indexed
        );
    }

    #[test]
    fn a_run_over_more_rows_than_one_chunk_sees_all_of_them() {
        let h = Harness::new();
        let dir = tempfile::TempDir::new().unwrap();
        let root = format!("{}/", dir.path().to_string_lossy());
        let total = CHUNK + 7;
        for i in 0..total {
            let name = format!("{i:05}.mp3");
            touch(&dir.path().join(&name));
            h.add(&format!("{root}{name}"));
        }

        let report = reconciler(&h).run(&[root]).unwrap();
        assert_eq!(report.seen, total, "the chunked paging dropped rows");
        assert_eq!(report.marked_missing, 0);
    }

    #[test]
    fn reconciling_twice_reports_nothing_further_to_do() {
        let h = Harness::new();
        let dir = tempfile::TempDir::new().unwrap();
        let root = format!("{}/", dir.path().to_string_lossy());
        touch(&dir.path().join("a.mp3"));
        h.add(&format!("{root}a.mp3"));

        let first = reconciler(&h).run(&[root.clone()]).unwrap();
        let second = reconciler(&h).run(&[root]).unwrap();
        assert_eq!(first.marked_missing, 0);
        assert_eq!(second.marked_missing, 0);
        assert_eq!(second.seen, 1);
    }

    #[test]
    fn an_empty_root_list_is_a_no_op() {
        let h = Harness::new();
        let id = h.add("/m/a.mp3");
        let report = reconciler(&h).run(&[]).unwrap();
        assert_eq!(report.seen, 0);
        assert_eq!(report.marked_missing, 0);
        assert_eq!(
            h.tracks.get_by_id(&id).unwrap().unwrap().index_status,
            IndexStatus::Indexed
        );
    }
}

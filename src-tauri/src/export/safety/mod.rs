//! The safety layer. Built first. Every rule exists because of a specific way
//! to destroy an irreplaceable library.
//!
//! The public surface is deliberately small and the order it must be used in is
//! enforced by [`super::run`], not by documentation: refuse while the app is
//! running, back up, write atomically, verify.

mod atomic;
mod backup;
mod process;

use std::path::Path;

pub use atomic::{assert_preserved, atomic_write, copy_modify_verify};
pub use backup::{backup_file, list_backups, restore_backup, BackupEntry};
pub use process::{enumerate_processes, lock_probe, matching, probe, running_processes};

use crate::error::{KeystoneError, Result};
use crate::export::ExportTarget;

/// Dated backups kept per target.
pub const KEEP_BACKUPS: usize = 10;

/// Process names that mean the target app is open.
///
/// Matching is by basename, case-insensitively, and a prefix counts: Traktor
/// ships as `Traktor Pro 3`, and a new point release must not open a hole in
/// the gate. False positives refuse a write that would have been fine, which is
/// the direction to be wrong in.
pub fn process_names(target: ExportTarget) -> &'static [&'static str] {
    match target {
        ExportTarget::Serato => &["Serato DJ Pro", "Serato DJ Lite"],
        ExportTarget::Traktor => &["Traktor", "Traktor Pro"],
        ExportTarget::Rekordbox => &["rekordbox", "rekordboxAgent"],
    }
}

/// Refuses while the DJ app is running.
///
/// For Traktor this is not politeness: Traktor holds the collection in memory
/// and rewrites `collection.nml` on exit, so any external edit made while it
/// runs is **silently discarded** — and a half-written file can be read in and
/// re-saved, laundering corruption into the backup rotation.
///
/// Process-name detection **plus** a lock probe, because a renamed binary or a
/// headless agent defeats the name check alone.
pub fn assert_not_running(target: ExportTarget, held_file: Option<&Path>) -> Result<()> {
    // An unanswerable check is refused rather than assumed clear. This returns
    // Err only when the OS would not produce a process list at all, which is
    // the one case where "nothing matched" and "nobody asked" look identical.
    let found = probe(target).map_err(|e| {
        KeystoneError::ExportRefused(format!(
            "Keystone could not check whether {} is running ({e}), and will not \
             write to a library it cannot prove is closed.",
            app_label(target)
        ))
    })?;
    if !found.is_empty() {
        return Err(KeystoneError::ExportRefused(refusal(target, &found)));
    }
    if let Some(path) = held_file {
        if path.exists() && lock_probe(path) {
            return Err(KeystoneError::ExportRefused(format!(
                "Another program is holding {}. Close {} and try again.",
                path.display(),
                app_label(target)
            )));
        }
    }
    Ok(())
}

/// The sentence the blocking modal shows. It names the app and says what would
/// happen, because "export failed" sends the user looking for a bug in Keystone.
fn refusal(target: ExportTarget, found: &[String]) -> String {
    let base = format!(
        "{} is running ({}). Quit it before exporting",
        app_label(target),
        found.join(", ")
    );
    match target {
        ExportTarget::Traktor => format!(
            "{base}: Traktor keeps its collection in memory and rewrites \
             collection.nml when it quits, so anything written now is discarded."
        ),
        ExportTarget::Rekordbox => format!(
            "{base}: rekordbox will not see a library file that changed while \
             it was open."
        ),
        ExportTarget::Serato => format!("{base}."),
    }
}

fn app_label(target: ExportTarget) -> &'static str {
    match target {
        ExportTarget::Rekordbox => "rekordbox",
        ExportTarget::Traktor => "Traktor",
        ExportTarget::Serato => "Serato DJ",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_refusal_says_what_would_happen_not_just_that_it_failed() {
        let msg = refusal(ExportTarget::Traktor, &["Traktor".into()]);
        assert!(msg.contains("Traktor is running"));
        assert!(msg.contains("discarded"), "{msg}");
    }

    #[test]
    fn a_destination_that_does_not_exist_yet_cannot_refuse_itself() {
        // The first rekordbox export writes a file nobody holds. If the gate
        // read a missing destination as locked, the feature would never run.
        let dest = std::path::Path::new("/keystone-no-such-dir/keystone.xml");
        assert!(!lock_probe(dest));
    }

    #[test]
    fn every_target_names_at_least_one_process() {
        for t in [
            ExportTarget::Rekordbox,
            ExportTarget::Traktor,
            ExportTarget::Serato,
        ] {
            assert!(!process_names(t).is_empty(), "{t:?} has no process names");
        }
    }
}

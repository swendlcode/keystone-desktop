//! Is the DJ app open? Two independent answers, because either one alone is
//! defeatable: the process table, and an exclusive open of the file itself.
//!
//! Process enumeration shells out rather than taking a dependency on a
//! process-listing crate. The whole check is one string compare against a
//! handful of names, it runs once per export rather than per track, and a new
//! crate in the tree is a licence surface for something `ps` already answers.

use std::path::Path;
use std::process::Command;

use crate::error::{KeystoneError, Result};
use crate::export::safety::process_names;
use crate::export::ExportTarget;

/// Matching names only. Empty means "nothing matched **or** the OS could not be
/// asked"; the write path uses [`probe`] instead, which tells those apart.
pub fn running_processes(target: ExportTarget) -> Vec<String> {
    probe(target).unwrap_or_else(|e| {
        tracing::warn!("process probe failed: {}", e);
        Vec::new()
    })
}

/// `Err` means the question could not be answered. The caller refuses on that
/// rather than assuming the app is closed: an unanswerable check is exactly the
/// case where a silent write costs someone their cue points.
pub fn probe(target: ExportTarget) -> Result<Vec<String>> {
    Ok(matching(&enumerate_processes()?, target))
}

/// Every running process, so the settings screen can ask about all three
/// targets without paying for three process listings.
pub fn enumerate_processes() -> Result<Vec<String>> {
    enumerate()
}

pub fn matching(all: &[String], target: ExportTarget) -> Vec<String> {
    let needles = process_names(target);
    let mut hit: Vec<String> = all
        .iter()
        .filter(|name| needles.iter().any(|n| is_match(name, n)))
        .cloned()
        .collect();
    hit.sort();
    hit.dedup();
    hit
}

/// Case-insensitive on the basename, and a prefix counts: Traktor ships as
/// `Traktor Pro 3` and a point release must not open a hole in the gate. The
/// cost of the loose match is refusing a write that would have been fine, which
/// is the direction to be wrong in.
fn is_match(process: &str, needle: &str) -> bool {
    let p = process.to_ascii_lowercase();
    let n = needle.to_ascii_lowercase();
    p == n || p.starts_with(&n)
}

#[cfg(unix)]
fn enumerate() -> Result<Vec<String>> {
    // `comm` is the executable path on macOS and the short name on Linux, so
    // the basename is taken in both cases.
    let out = Command::new("ps")
        .args(["-A", "-o", "comm="])
        .output()
        .map_err(|e| KeystoneError::Export(format!("could not run ps: {e}")))?;
    if !out.status.success() {
        return Err(KeystoneError::Export("ps returned no process list".into()));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.trim().rsplit('/').next())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect())
}

#[cfg(windows)]
fn enumerate() -> Result<Vec<String>> {
    use std::os::windows::process::CommandExt;
    /// CREATE_NO_WINDOW. Without it every export flashes a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let out = Command::new("tasklist")
        .args(["/NH", "/FO", "CSV"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| KeystoneError::Export(format!("could not run tasklist: {e}")))?;
    if !out.status.success() {
        return Err(KeystoneError::Export(
            "tasklist returned no process list".into(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.split('"').nth(1))
        .map(|image| image.trim_end_matches(".exe").trim_end_matches(".EXE"))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect())
}

/// Attempts an exclusive open. `true` means something else holds the file.
///
/// This is the weaker of the two checks on unix, where none of these apps take
/// an advisory lock and a read-write open therefore succeeds even with Traktor
/// open — the process scan is the real gate there. On Windows the zero share
/// mode is decisive, which is the platform where the sqlite-backed targets
/// actually hold their files.
pub fn lock_probe(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        return std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(path)
            .is_err();
    }
    #[cfg(not(windows))]
    {
        // Missing is not locked: the first export of a file we are about to
        // create must not refuse itself.
        if !path.exists() {
            return false;
        }
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .is_err()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_match_is_a_case_insensitive_prefix_on_the_basename() {
        assert!(is_match("Traktor Pro 3", "Traktor"));
        assert!(is_match("traktor", "Traktor"));
        assert!(is_match("rekordboxAgent", "rekordbox"));
        // Not a substring match: a process that merely mentions the name is not
        // the app, and refusing every export on it would be unexplainable.
        assert!(!is_match("my-rekordbox-helper", "rekordbox"));
    }

    #[test]
    fn enumeration_answers_on_this_machine() {
        // The gate is only as good as the enumeration, and a silently empty
        // list is the failure mode that lets a write through with Traktor open.
        let all = enumerate().expect("the OS must answer a process list");
        assert!(!all.is_empty());
    }

    #[test]
    fn a_missing_file_is_not_a_lock() {
        assert!(!lock_probe(Path::new(
            "/keystone-no-such-dir/collection.nml"
        )));
    }
}

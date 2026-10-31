//! Dated copies of the DJ apps' collection files, taken before the first write
//! to each target. This is the only thing that matters when it goes wrong,
//! which is why "Restore backup" is in Settings rather than hidden.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{KeystoneError, Result};
use crate::export::safety::{atomic_write, KEEP_BACKUPS};
use crate::export::ExportTarget;

/// Serialized straight to the frontend: the restore list has to show *when* a
/// backup was taken, and a bare path cannot say that — the timestamp in the
/// filename is basic-format ISO and not something a UI should be parsing back.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub path: PathBuf,
    pub target: ExportTarget,
    pub created_unix: i64,
    pub size_bytes: u64,
}

/// `<app data>/backups/<target>/<name>-<timestamp>.<ext>`.
///
/// The timestamp is ISO 8601 basic format (`20261031T142530Z`) rather than
/// extended: a colon is not a legal filename character on Windows, and a backup
/// the user cannot restore on the machine that made it is not a backup. Basic
/// format still sorts chronologically as a string, which is what the pruning
/// relies on.
pub fn backup_file(src: &Path, backups_dir: &Path, target: ExportTarget) -> Result<PathBuf> {
    let dir = backups_dir.join(target.as_str());
    std::fs::create_dir_all(&dir)?;

    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| target.as_str().to_string());
    let ext = src.extension().map(|e| e.to_string_lossy().into_owned());
    let stamp = iso8601_basic_utc(SystemTime::now());
    let name = match &ext {
        Some(ext) => format!("{stem}-{stamp}.{ext}"),
        None => format!("{stem}-{stamp}"),
    };
    let dest = dir.join(name);

    // Copied through the atomic path, so an interrupted backup is never left
    // looking like a complete one in the rotation.
    let bytes = std::fs::read(src)?;
    atomic_write(&dest, |f| {
        use std::io::Write;
        f.write_all(&bytes)?;
        Ok(())
    })?;
    prune(&dir, KEEP_BACKUPS)?;
    Ok(dest)
}

pub fn list_backups(backups_dir: &Path, target: ExportTarget) -> Result<Vec<BackupEntry>> {
    let dir = backups_dir.join(target.as_str());
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        out.push(BackupEntry {
            path: entry.path(),
            target,
            created_unix: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            size_bytes: meta.len(),
        });
    }
    out.sort_by(|a, b| b.created_unix.cmp(&a.created_unix));
    Ok(out)
}

/// Restoring is a write like any other, so it goes through the same atomic
/// path: a crash halfway through a restore would otherwise destroy the file the
/// user was trying to rescue.
pub fn restore_backup(backup: &Path, dest: &Path) -> Result<()> {
    if !backup.is_file() {
        return Err(KeystoneError::NotFound(format!(
            "backup {} is gone",
            backup.display()
        )));
    }
    let bytes = std::fs::read(backup)?;
    atomic_write(dest, |f| {
        use std::io::Write;
        f.write_all(&bytes)?;
        Ok(())
    })
}

/// Oldest first by name, which is chronological because of the timestamp
/// format. Pruning failures are not fatal: a full rotation is a housekeeping
/// problem, and failing the export over it would be worse than keeping an
/// extra copy.
fn prune(dir: &Path, keep: usize) -> Result<()> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    if files.len() <= keep {
        return Ok(());
    }
    files.sort();
    for old in files.iter().take(files.len() - keep) {
        if let Err(e) = std::fs::remove_file(old) {
            tracing::warn!("could not prune backup {}: {}", old.display(), e);
        }
    }
    Ok(())
}

/// `20261031T142530Z`. Written by hand because the dependency table carries no
/// date crate, and this is the only place in the app that needs a calendar.
fn iso8601_basic_utc(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant's `civil_from_days`, the standard shift-the-epoch-to-March
/// algorithm: with March as month 1 the leap day falls at the end of the year
/// and the month-length pattern becomes a single linear expression.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_stamp_matches_known_instants_and_carries_no_illegal_character() {
        let at = |s: u64| iso8601_basic_utc(UNIX_EPOCH + Duration::from_secs(s));
        assert_eq!(at(0), "19700101T000000Z");
        assert_eq!(at(1_000_000_000), "20010909T014640Z");
        // A leap day, which is where a hand-rolled calendar goes wrong.
        assert_eq!(at(1_583_020_800), "20200301T000000Z");
        assert!(!at(1_762_000_000).contains(':'), "illegal on Windows");
    }

    #[test]
    fn backups_are_dated_restorable_and_pruned_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("collection.nml");
        std::fs::write(&src, b"<NML/>").unwrap();
        let backups = dir.path().join("backups");

        let first = backup_file(&src, &backups, ExportTarget::Traktor).unwrap();
        assert!(first.exists());
        assert_eq!(std::fs::read(&first).unwrap(), b"<NML/>");
        assert!(first.to_string_lossy().contains("collection-"));

        let listed = list_backups(&backups, ExportTarget::Traktor).unwrap();
        assert_eq!(listed.len(), 1);

        std::fs::write(&src, b"<NML>changed</NML>").unwrap();
        restore_backup(&first, &src).unwrap();
        assert_eq!(std::fs::read(&src).unwrap(), b"<NML/>");
    }

    #[test]
    fn the_rotation_keeps_the_newest_n() {
        let dir = tempfile::tempdir().unwrap();
        let target_dir = dir.path().join("traktor");
        std::fs::create_dir_all(&target_dir).unwrap();
        let total = KEEP_BACKUPS + 3;
        for i in 0..total {
            std::fs::write(
                target_dir.join(format!("collection-202610{i:02}T000000Z.nml")),
                b"x",
            )
            .unwrap();
        }
        prune(&target_dir, KEEP_BACKUPS).unwrap();

        let left: Vec<String> = list_backups(dir.path(), ExportTarget::Traktor)
            .unwrap()
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left.len(), KEEP_BACKUPS);
        // The three oldest went, and they are the three oldest by date because
        // the timestamp format sorts that way.
        for gone in 0..3 {
            let name = format!("collection-202610{gone:02}T000000Z.nml");
            assert!(!left.contains(&name), "{name} should have been pruned");
        }
        for kept in 3..total {
            let name = format!("collection-202610{kept:02}T000000Z.nml");
            assert!(left.contains(&name), "{name} should have survived");
        }
    }
}

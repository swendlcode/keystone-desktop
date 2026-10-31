//! S0 — the walk. Audio extensions only; a DJ library has no MIDI or presets in
//! it, and the skip list keeps the walk out of the directories that are never
//! anybody's music.

use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

use crate::error::Result;

/// Everything symphonia can decode that a DJ library actually contains. Ogg
/// Vorbis is readable but is never a Serato export target; that restriction
/// lives in the exporter, not here.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "mp4", "aac", "aiff", "aif", "aifc", "wav", "wave", "flac", "ogg", "oga", "opus",
    "alac",
];

pub const SKIP_DIRS: &[&str] = &[
    ".git",
    ".svn",
    "node_modules",
    ".Trash",
    "$RECYCLE.BIN",
    "System Volume Information",
    // The DJ apps' own working directories. Their contents are caches and
    // databases, and walking them would index Serato's own temp copies.
    "_Serato_",
    "PIONEER",
    "Native Instruments",
];

#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_unix: i64,
}

#[derive(Debug, Clone, Default)]
pub struct ScanStats {
    pub files_seen: usize,
    pub audio_found: usize,
    pub dirs_skipped: usize,
}

pub struct Scanner;

impl Scanner {
    pub fn scan(root: &Path) -> Result<(Vec<ScannedFile>, ScanStats)> {
        let mut found = Vec::new();
        let mut stats = ScanStats::default();

        // `follow_links` stays off: a DJ library with an alias back to its own
        // parent would otherwise walk for ever, and a symlinked duplicate would
        // index the same audio under two paths.
        let walk = WalkDir::new(root).follow_links(false).into_iter();
        for entry in walk.filter_entry(|e| {
            if is_skipped_dir(e) {
                return false;
            }
            true
        }) {
            // A directory the user cannot read is skipped, not fatal: one
            // permission-denied folder must not abandon the rest of the walk.
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::debug!("skipping unreadable entry: {}", e);
                    stats.dirs_skipped += 1;
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            stats.files_seen += 1;

            let Some(extension) = audio_extension(entry.path()) else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            stats.audio_found += 1;
            found.push(ScannedFile {
                path: entry.path().to_path_buf(),
                extension,
                size_bytes: meta.len(),
                modified_unix: modified_unix(&meta),
            });
        }

        Ok((found, stats))
    }
}

pub fn is_audio(ext: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&ext)
}

/// Lower-cased extension, or `None` when this is not a file we can decode.
///
/// Lower-casing matters more than it looks: `.MP3` and `.AIFF` are common in
/// libraries that came off a Windows machine or out of a CD ripper, and a
/// case-sensitive check silently ignores them.
pub fn audio_extension(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    is_audio(&ext).then_some(ext)
}

fn is_skipped_dir(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    // The root itself is never skipped: a user who points Keystone at a folder
    // called `_Serato_` means it.
    if entry.depth() == 0 {
        return false;
    }
    let Some(name) = entry.file_name().to_str() else {
        return false;
    };
    SKIP_DIRS.iter().any(|d| d.eq_ignore_ascii_case(name))
}

fn modified_unix(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"not really audio").unwrap();
    }

    #[test]
    fn the_walk_finds_audio_at_any_depth_and_ignores_everything_else() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        for rel in [
            "a.mp3",
            "House/b.AIFF",
            "House/Deep/c.flac",
            "notes.txt",
            "artwork/cover.jpg",
            "House/.DS_Store",
        ] {
            touch(&root.join(rel));
        }

        let (found, stats) = Scanner::scan(root).unwrap();
        let mut names: Vec<String> = found
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["a.mp3", "b.AIFF", "c.flac"]);
        assert_eq!(stats.audio_found, 3);
        assert_eq!(stats.files_seen, 6);

        // Upper-case extensions are normalised, because a library off a Windows
        // machine is full of them and a case-sensitive check loses the lot.
        let aiff = found.iter().find(|f| f.path.ends_with("b.AIFF")).unwrap();
        assert_eq!(aiff.extension, "aiff");
    }

    #[test]
    fn the_skip_list_keeps_the_walk_out_of_the_dj_apps_own_directories() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        touch(&root.join("keep.mp3"));
        for rel in [
            "_Serato_/temp.mp3",
            "PIONEER/rekordbox/x.mp3",
            "node_modules/pkg/y.mp3",
            ".Trash/deleted.mp3",
        ] {
            touch(&root.join(rel));
        }

        let (found, _) = Scanner::scan(root).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("keep.mp3"));
    }

    #[test]
    fn a_root_the_user_pointed_at_is_walked_even_if_it_is_named_like_a_skip() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().join("_Serato_");
        touch(&root.join("inside.mp3"));
        let (found, _) = Scanner::scan(&root).unwrap();
        assert_eq!(found.len(), 1, "the root itself must not be skipped");
    }

    #[test]
    fn each_file_carries_the_size_and_mtime_the_incremental_rescan_needs() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("a.mp3");
        touch(&path);
        let (found, _) = Scanner::scan(dir.path()).unwrap();
        assert_eq!(found[0].size_bytes, 16);
        assert!(found[0].modified_unix > 0);
    }

    #[test]
    fn a_missing_root_is_an_empty_walk_rather_than_an_error() {
        // An unmounted external drive is the everyday case, and it must not
        // fail the scan command.
        let (found, stats) = Scanner::scan(Path::new("/definitely/not/here")).unwrap();
        assert!(found.is_empty());
        assert_eq!(stats.audio_found, 0);
    }
}

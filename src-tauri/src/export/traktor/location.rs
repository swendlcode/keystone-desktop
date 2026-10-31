//! The match key, and where the collection file lives.
//!
//! `LOCATION` is the only thing tying an NML entry to a file on disk, and
//! getting it wrong puts cues on the wrong song. It is three attributes, not a
//! path: `@VOLUME` (the disk), `@DIR` (which uses **`/:` as the path
//! separator**, not `/`) and `@FILE`.

use std::path::{Path, PathBuf};

/// Traktor's own separator inside `@DIR`.
pub const DIR_SEPARATOR: &str = "/:";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocationKey {
    pub dir: String,
    pub file: String,
    /// Empty when the volume could not be determined. An empty volume is a
    /// weaker key, and [`super::rewrite`] only falls back to matching without
    /// it when exactly one candidate remains.
    pub volume: String,
}

impl LocationKey {
    /// Built from the attributes as the file carries them, with no
    /// normalisation: comparing what Traktor wrote against what Traktor wrote
    /// is the only comparison that cannot drift.
    pub fn from_attrs(dir: &str, file: &str, volume: &str) -> Self {
        Self {
            dir: dir.to_string(),
            file: file.to_string(),
            volume: volume.to_string(),
        }
    }

    pub fn has_volume(&self) -> bool {
        !self.volume.is_empty()
    }

    /// Volume-blind identity, used only as the last-resort match.
    pub fn dir_file(&self) -> (String, String) {
        (self.dir.clone(), self.file.clone())
    }
}

/// Splits an absolute path into Traktor's triplet.
///
/// Both platform shapes are handled from the string rather than from
/// `std::path`, so a Windows path can be tested on a unix host and vice versa:
/// the format is Traktor's, not the host's.
pub fn location_key(path: &Path) -> LocationKey {
    let raw = path.to_string_lossy().replace('\\', "/");
    if let Some((drive, rest)) = windows_drive(&raw) {
        // Traktor records the drive specifier including the colon.
        return split(&format!("{drive}:"), rest);
    }
    if let Some(rest) = raw.strip_prefix("/Volumes/") {
        if let Some((volume, tail)) = rest.split_once('/') {
            return split(volume, tail);
        }
    }
    // The startup disk. Traktor writes its name, which is the one piece of the
    // triplet the path itself does not contain.
    split(&boot_volume_name().unwrap_or_default(), raw.trim_start_matches('/'))
}

fn windows_drive(raw: &str) -> Option<(char, &str)> {
    let mut chars = raw.chars();
    let letter = chars.next()?;
    if !letter.is_ascii_alphabetic() || chars.next()? != ':' {
        return None;
    }
    Some((letter, raw.get(3..).unwrap_or("")))
}

/// `relative` is already volume-relative and `/`-separated.
fn split(volume: &str, relative: &str) -> LocationKey {
    let mut parts: Vec<&str> = relative.split('/').filter(|s| !s.is_empty()).collect();
    let file = parts.pop().unwrap_or("").to_string();
    let dir = if parts.is_empty() {
        DIR_SEPARATOR.to_string()
    } else {
        format!(
            "{DIR_SEPARATOR}{}{DIR_SEPARATOR}",
            parts.join(DIR_SEPARATOR)
        )
    };
    LocationKey::from_attrs(&dir, &file, volume)
}

/// The startup disk's name, which on macOS is the `/Volumes` entry that
/// resolves to `/`. Returns `None` elsewhere and on failure, and an unknown
/// volume is honestly empty rather than guessed: a wrong volume name is a
/// mismatch, and a mismatch is a cue on the wrong song.
pub fn boot_volume_name() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    for entry in std::fs::read_dir("/Volumes").ok()?.flatten() {
        // One unreadable volume must not stop the search: an external disk that
        // just went away is the common case.
        if entry.path().canonicalize().ok().as_deref() == Some(Path::new("/")) {
            return Some(entry.file_name().to_string_lossy().into_owned());
        }
    }
    None
}

/// Traktor creates a **new root directory on every version update**, so glob the
/// parent and pick by version then mtime. The root is also user-overridable in
/// Preferences -> File Management -> Directories, which is why the spec's
/// `CHANGE` button is mandatory rather than a nicety.
pub fn detect_collection() -> Option<PathBuf> {
    let parent = super::super::detect::home()?
        .join("Documents")
        .join("Native Instruments");
    let mut candidates: Vec<(Vec<u32>, std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&parent).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("Traktor") {
            continue;
        }
        let nml = entry.path().join("collection.nml");
        if !nml.is_file() {
            continue;
        }
        let mtime = nml
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        candidates.push((version_parts(&name), mtime, nml));
    }
    // Highest version wins; the most recently written collection breaks a tie,
    // because a user who downgraded is working in the older install.
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    candidates.pop().map(|(_, _, path)| path)
}

/// `Traktor 4.2.0` -> `[4, 2, 0]`. A folder with no version sorts first, so a
/// real versioned install always beats it.
fn version_parts(name: &str) -> Vec<u32> {
    name.split_whitespace()
        .last()
        .map(|v| v.split('.').filter_map(|p| p.parse().ok()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dir_attribute_uses_traktors_own_separator() {
        let key = location_key(Path::new("/Volumes/Music/DJ/House/track.mp3"));
        assert_eq!(key.volume, "Music");
        assert_eq!(key.dir, "/:DJ/:House/:");
        assert_eq!(key.file, "track.mp3");
    }

    #[test]
    fn a_windows_path_keeps_the_drive_specifier_as_the_volume() {
        let key = location_key(Path::new(r"D:\Music\House\track.mp3"));
        assert_eq!(key.volume, "D:");
        assert_eq!(key.dir, "/:Music/:House/:");
        assert_eq!(key.file, "track.mp3");
    }

    #[test]
    fn a_file_at_the_volume_root_still_has_a_dir() {
        let key = location_key(Path::new("/Volumes/USB/track.mp3"));
        assert_eq!(key.dir, "/:");
        assert_eq!(key.file, "track.mp3");
    }

    #[test]
    fn a_startup_disk_path_keeps_its_directories_whatever_the_volume_is_called() {
        let key = location_key(Path::new("/Users/dj/Music/track.mp3"));
        assert_eq!(key.dir, "/:Users/:dj/:Music/:");
        assert_eq!(key.file, "track.mp3");
    }

    #[test]
    fn versions_sort_numerically_rather_than_as_strings() {
        assert!(version_parts("Traktor 4.2.0") > version_parts("Traktor 3.11.1"));
        assert!(version_parts("Traktor 3.11.1") > version_parts("Traktor 3.2.0"));
        assert!(version_parts("Traktor") < version_parts("Traktor 2.0.0"));
    }
}

//! Path identity and the move itself.
//!
//! Two hazards live here, and both produce silent wrong behaviour rather than
//! an error:
//!
//! - **Case-insensitive filesystems.** APFS is case-insensitive by default, so
//!   `track.mp3` -> `Track.mp3` is either a no-op or an error depending on the
//!   call. It goes via a temporary name.
//! - **NFD vs NFC.** macOS stores decomposed filenames, so a string compare
//!   against a composed name says "different file" when it is the same file.
//!   Identity is asked of the filesystem through `same-file`; strings are only
//!   ever compared after normalising.

use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

use crate::error::Result;
use crate::tags::tag_err;

/// Identity, not string equality. Uses `same-file`, so a hard link, a symlink
/// and an NFD/NFC spelling of the same file all compare equal.
///
/// Falls back to a normalised string compare only when the question cannot be
/// put to the filesystem — which, during a dry run over a plan, is the common
/// case rather than the exception.
pub fn is_same_file(a: &Path, b: &Path) -> bool {
    match same_file::is_same_file(a, b) {
        Ok(same) => same,
        Err(_) => same_path_text(a, b),
    }
}

/// True when the two names differ only by case, which needs the two-step
/// rename on a case-insensitive filesystem.
pub fn is_case_only_change(from: &Path, to: &Path) -> bool {
    let (Some(a), Some(b)) = (component(from), component(to)) else {
        return false;
    };
    a != b && a.to_lowercase() == b.to_lowercase()
}

/// Compares two paths as text, normalised to NFC first. This is the comparison
/// for "is the row already pointing at this name", never for "is this the same
/// file" — that is [`is_same_file`]'s job.
pub fn same_path_text(a: &Path, b: &Path) -> bool {
    nfc(&a.to_string_lossy()) == nfc(&b.to_string_lossy())
}

pub fn nfc(value: &str) -> String {
    value.nfc().collect()
}

fn component(path: &Path) -> Option<String> {
    path.file_name().map(|n| nfc(&n.to_string_lossy()))
}

/// Renames within one directory.
///
/// A case-only change goes through a temporary name, because on APFS and NTFS
/// the direct rename either does nothing or fails, and both outcomes look like
/// success to a caller that only checks for an error.
pub fn move_file(from: &Path, to: &Path) -> Result<()> {
    if is_case_only_change(from, to) {
        let staging = staging_path(to);
        std::fs::rename(from, &staging)?;
        // If this second step fails the file is sitting under the staging name,
        // which is why the journal row is still open: recovery finds it.
        return std::fs::rename(&staging, to).map_err(Into::into);
    }

    if to.exists() && !is_same_file(from, to) {
        return Err(tag_err(to, "already exists"));
    }
    std::fs::rename(from, to).map_err(Into::into)
}

/// A name in the same directory that nothing else can hold. The process id and
/// a nanosecond stamp are enough: the only competitor would be another Keystone
/// renaming the same file at the same moment.
fn staging_path(to: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    to.with_file_name(format!(".keystone-{}-{stamp}-{name}", std::process::id()))
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;

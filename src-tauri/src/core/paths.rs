//! Lexical path normalisation.
//!
//! A stored path is matched against a watched folder by **string prefix**
//! (`existing_paths`, `mark_missing_under`, the scope filters in
//! `query_builder`), all of which lean on `idx_tracks_path` as a range scan.
//! That makes an unnormalised path worse than untidy: it is unreachable.
//!
//! A playlist in `<crate>/Playlists/` listing `../Songs/Vol. 2/track.mp3`
//! resolved to `<crate>/Playlists/../Songs/Vol. 2/track.mp3`, which the OS
//! opens happily. But the watched folder is `<crate>/Songs/`, and
//! `"<crate>/Playlists/.."` sorts before `"<crate>/Songs/"`, so not one of the
//! fifty imported tracks fell inside the root that owned them: the reconciler
//! never saw them, a rescan never touched them, and the library read as empty
//! while fifty rows sat in the table.
//!
//! Normalisation here is **lexical** — no `canonicalize`, no stat, no symlink
//! resolution. `canonicalize` would rewrite a path the user recognises into
//! `/private/var/...`, and would resolve a symlinked library onto its target,
//! which is a different path from the one they added. The cost is that
//! `a/b/../c` is only the same as `a/c` when `b` is a real directory; for a
//! playlist pointing at a sibling folder of its own, which is the case this
//! exists for, it is.

use std::path::{Component, Path, PathBuf};

/// Removes `.` and `..` components and collapses repeated separators.
///
/// A leading `..` on a relative path is kept: there is nothing above it to
/// cancel, and silently dropping it would change which file is meant.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    // Tracks how many trailing components may be cancelled by a `..`. Only
    // `Normal` ones may: popping a root or a prefix would walk off the top.
    let mut popable = 0usize;
    let mut rooted = false;

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if popable > 0 {
                    out.pop();
                    popable -= 1;
                } else if !rooted {
                    // Relative path with no parent recorded yet, e.g. `../x`.
                    // On an absolute path there is nothing above the root, so
                    // `/..` is `/` and the component is simply dropped.
                    out.push("..");
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                rooted = true;
                out.push(component.as_os_str());
            }
            Component::Normal(part) => {
                out.push(part);
                popable += 1;
            }
        }
    }

    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// True when `normalize` would change the path, which is the cheap test the
/// repair pass and the importer both gate on.
pub fn is_normalized(path: &str) -> bool {
    normalize(Path::new(path)) == Path::new(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(s: &str) -> String {
        normalize(Path::new(s)).to_string_lossy().into_owned()
    }

    /// The exact shape the m3u importer produced, and the reason a watched
    /// folder could not see its own tracks.
    #[test]
    fn a_playlist_relative_entry_resolves_to_the_real_folder() {
        assert_eq!(
            norm("/crate/Playlists/../Songs/Vol. 2/CamelPhat & Elderbrook - Cola.mp3"),
            "/crate/Songs/Vol. 2/CamelPhat & Elderbrook - Cola.mp3"
        );
    }

    #[test]
    fn the_normalized_path_falls_under_the_watched_root() {
        let root = "/crate/Songs/";
        let raw = "/crate/Playlists/../Songs/a.mp3";
        assert!(
            !raw.starts_with(root),
            "precondition: the raw path is outside the root"
        );
        assert!(norm(raw).starts_with(root));
    }

    #[test]
    fn dots_and_doubled_separators_go() {
        assert_eq!(norm("/a/./b//c"), "/a/b/c");
        assert_eq!(norm("/a/b/../../c"), "/c");
        assert_eq!(norm("/a/b/c/.."), "/a/b");
    }

    /// Never walks off the top: a `..` at the root has nothing to cancel.
    #[test]
    fn parent_of_the_root_is_still_the_root() {
        assert_eq!(norm("/.."), "/");
        assert_eq!(norm("/../../a"), "/a");
    }

    /// A relative `..` is meaningful and must survive.
    #[test]
    fn a_leading_parent_on_a_relative_path_is_kept() {
        assert_eq!(norm("../Songs/a.mp3"), "../Songs/a.mp3");
        assert_eq!(norm("./../a.mp3"), "../a.mp3");
    }

    #[test]
    fn an_already_clean_path_is_untouched() {
        for p in [
            "/crate/Songs/a.mp3",
            "/Users/dj/Music/Vol. 2/Fred again.. - Delilah.mp3",
        ] {
            assert_eq!(norm(p), p);
            assert!(is_normalized(p));
        }
        assert!(!is_normalized("/crate/Playlists/../Songs/a.mp3"));
    }

    /// A dotted *filename* is not a `..` component and must not be touched.
    #[test]
    fn a_dotted_filename_survives() {
        assert_eq!(
            norm("/music/Fred again.. - Delilah.mp3"),
            "/music/Fred again.. - Delilah.mp3"
        );
    }
}

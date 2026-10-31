//! Settings persistence: the whole SPEC.md §3 surface as one JSON file.
//!
//! A file rather than a table because the surface is large but flat and
//! single-user, and a settings read happens on every tempo format. The one rule
//! that matters is that a bad file can never stop the app launching: a user with
//! a corrupt `settings.json` needs Keystone more than they need their column
//! layout.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::analysis::AnalysisContext;
use crate::error::Result;
use crate::models::Settings;

/// Reads `settings.json`, falling back to defaults on anything at all.
///
/// A file that will not parse is moved aside rather than overwritten. It is the
/// only copy of what the user chose, and the next save would destroy it — which
/// also destroys the evidence of whatever wrote it badly.
pub fn load(path: &Path) -> Settings {
    if !path.exists() {
        return Settings::default();
    }
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => {
            tracing::warn!("settings could not be read ({}); using defaults", e);
            return Settings::default();
        }
    };
    match serde_json::from_str::<Settings>(&raw) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::warn!("settings would not parse ({}); using defaults", e);
            quarantine(path);
            Settings::default()
        }
    }
}

/// Writes atomically: a temp file in the same directory, then a rename over the
/// target. A half-written `settings.json` is the corrupt file this module is
/// built to survive, so it must not be this module that creates one.
pub fn save(path: &Path, settings: &Settings) -> Result<()> {
    let json = serde_json::to_vec_pretty(settings)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(&json)?;
    tmp.as_file().sync_all()?;
    // `persist` rather than `fs::rename`, which fails on Windows when the
    // destination exists.
    tmp.persist(path)
        .map_err(|e| crate::error::KeystoneError::Io(e.error))?;
    Ok(())
}

/// Moves an unparseable file to `settings.json.corrupt`, keeping one generation.
fn quarantine(path: &Path) {
    let aside: PathBuf = path.with_extension("json.corrupt");
    if let Err(e) = std::fs::rename(path, &aside) {
        tracing::warn!("could not set the corrupt settings file aside: {}", e);
    }
}

/// A fresh analysis context for changed settings, carrying the loaded model
/// session over.
///
/// The ONNX session is expensive to build and cheap to call, so it is shared
/// across every worker and must survive a settings change — otherwise toggling
/// a checkbox in Display Options reloads the key model.
pub fn rebuild_context(previous: &AnalysisContext, settings: &Settings) -> AnalysisContext {
    AnalysisContext {
        skey: previous.skey.clone(),
        beat_arbiter: previous.beat_arbiter,
        ..AnalysisContext::new(previous.peaks_dir.clone(), settings.clone())
    }
}

/// True when `tempo_display` has to be re-folded from `tempo_raw`.
///
/// Only the range matters. Decimal places are a format applied at render time,
/// and the three tempo columns exist precisely so neither costs a re-analysis.
pub fn tempo_range_changed(previous: &Settings, next: &Settings) -> bool {
    previous.tempo_range() != next.tempo_range()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_corrupt_file_is_set_aside_rather_than_blocking_launch() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"{ this is not json").expect("write the bad file");

        let settings = load(&path);
        assert_eq!(settings.tempo_min, Settings::default().tempo_min);
        assert!(
            path.with_extension("json.corrupt").exists(),
            "the only copy of the user's choices must be kept"
        );
    }

    #[test]
    fn a_saved_file_round_trips() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("settings.json");
        let mut written = Settings::default();
        written.tempo_min = 60.0;
        written.start_at_quarter = true;
        save(&path, &written).expect("save");

        let read_back = load(&path);
        assert_eq!(read_back.tempo_min, 60.0);
        assert!(read_back.start_at_quarter);
    }

    #[test]
    fn only_the_range_forces_a_refold() {
        let base = Settings::default();
        let decimals = Settings {
            tempo_decimals: 2,
            ..base.clone()
        };
        assert!(!tempo_range_changed(&base, &decimals));

        let narrower = Settings {
            tempo_min: 90.0,
            ..base.clone()
        };
        assert!(tempo_range_changed(&base, &narrower));

        let unrestricted = Settings {
            restrict_tempo_range: false,
            ..base.clone()
        };
        assert!(tempo_range_changed(&base, &unrestricted));
    }
}

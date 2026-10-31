//! Target detection, the default destinations, and the copy the export screen
//! shows.
//!
//! Detection is best-effort on purpose and the spec's `CHANGE` button is
//! mandatory rather than a nicety: Traktor makes a new root directory on every
//! version update and the root is user-overridable in its own preferences.
//!
//! The manual-steps copy lives here because it is part of the feature, not
//! decoration. Keystone cannot push anything into rekordbox and cannot update
//! Serato's library cache; a screen that implies otherwise turns a working
//! export into a support ticket.

use std::path::PathBuf;

use crate::export::{safety, traktor, ExportTarget, TargetStatus};
use crate::models::Settings;

/// The sentence the Serato checkbox is disabled with. Saying it plainly beats a
/// checkbox that implies something the app does not do yet.
pub const UNSUPPORTED_SERATO: &str =
    "Serato export arrives in a later version. It rewrites the audio files \
     themselves, so it ships once the two library-file targets have proven the \
     safety layer.";

pub fn all_targets(settings: &Settings) -> Vec<TargetStatus> {
    // One process listing for all three targets: this runs every time the
    // settings screen opens, and the checkbox needs to read "quit Traktor
    // first" before the user clicks rather than after.
    let running = safety::enumerate_processes().unwrap_or_default();
    [
        ExportTarget::Rekordbox,
        ExportTarget::Traktor,
        ExportTarget::Serato,
    ]
    .into_iter()
    .map(|t| status(t, settings, &running))
    .collect()
}

fn status(target: ExportTarget, settings: &Settings, running: &[String]) -> TargetStatus {
    let override_path = match target {
        ExportTarget::Rekordbox => settings.rekordbox_path.clone(),
        ExportTarget::Traktor => settings.traktor_path.clone(),
        ExportTarget::Serato => settings.serato_path.clone(),
    }
    .filter(|p| !p.is_empty());

    let detected = default_destination(target);
    let installed = is_installed(target);
    TargetStatus {
        target,
        installed,
        detected_path: detected.map(|p| p.to_string_lossy().into_owned()),
        override_path,
        reason_unavailable: reason_unavailable(target, installed),
        app_running: !safety::matching(running, target).is_empty(),
        supported: target.supported_in_v1(),
        manual_steps: manual_steps(target),
    }
}

fn reason_unavailable(target: ExportTarget, installed: bool) -> Option<String> {
    if !target.supported_in_v1() {
        return Some(UNSUPPORTED_SERATO.to_string());
    }
    if installed {
        return None;
    }
    Some(match target {
        ExportTarget::Rekordbox => "rekordbox was not found on this machine. \
             Keystone can still write the XML file anywhere you choose."
            .to_string(),
        ExportTarget::Traktor => "No Traktor collection.nml was found. Traktor moves its folder on \
             every version update and the location is settable in its own \
             preferences, so point Keystone at it with CHANGE."
            .to_string(),
        ExportTarget::Serato => UNSUPPORTED_SERATO.to_string(),
    })
}

/// Where the run writes when the user has not chosen.
///
/// rekordbox gets a suggestion because the file is ours to put anywhere and
/// nothing of the user's is touched. Traktor gets detection or nothing: a
/// guessed path is one we would then rewrite, and that is how cues land in the
/// wrong library.
pub fn default_destination(target: ExportTarget) -> Option<PathBuf> {
    match target {
        ExportTarget::Rekordbox => Some(
            home()?
                .join("Music")
                .join("Keystone")
                .join("keystone-rekordbox.xml"),
        ),
        ExportTarget::Traktor => traktor::detect_collection(),
        ExportTarget::Serato => None,
    }
}

fn is_installed(target: ExportTarget) -> bool {
    match target {
        ExportTarget::Rekordbox => rekordbox_installed(),
        ExportTarget::Traktor => traktor::detect_collection().is_some(),
        ExportTarget::Serato => serato_installed(),
    }
}

/// The application itself on macOS (its folder carries the major version), and
/// the per-user data folder on Windows. Either one answers "is this a rekordbox
/// machine", which is all the checkbox needs.
fn rekordbox_installed() -> bool {
    #[cfg(target_os = "macos")]
    {
        if let Ok(entries) = std::fs::read_dir("/Applications") {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_ascii_lowercase();
                if name.starts_with("rekordbox") {
                    return true;
                }
            }
        }
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(home) = home() {
        candidates.push(home.join("Library/Pioneer/rekordbox"));
        candidates.push(home.join("Library/Application Support/Pioneer"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidates.push(PathBuf::from(appdata).join("Pioneer").join("rekordbox"));
    }
    candidates.iter().any(|p| p.exists())
}

/// `_Serato_` beside the user's Music folder. Detection only: v1 refuses the
/// write either way, and the status exists so the disabled checkbox can explain
/// itself to someone who does have Serato installed.
fn serato_installed() -> bool {
    home()
        .map(|h| h.join("Music").join("_Serato_").exists())
        .unwrap_or(false)
}

pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// What the user must do in the DJ app afterwards, in order.
///
/// rekordbox's list is long because the import is entirely manual: Keystone
/// writes a file and rekordbox has to be told to read it. Writing "exported
/// successfully" without these steps is how the feature gets reported as
/// broken by people for whom it worked perfectly.
pub fn manual_steps(target: ExportTarget) -> Vec<String> {
    match target {
        ExportTarget::Rekordbox => vec![
            "Keystone writes an XML file. It cannot push anything into rekordbox, \
             so nothing changes until you import it."
                .into(),
            "In rekordbox: Preferences -> Advanced -> Database -> rekordbox xml, \
             and set Imported Library to the file Keystone just wrote."
                .into(),
            "Turn on the rekordbox xml panel under View -> Layout so the imported \
             library is visible in the tree."
                .into(),
            format!(
                "Right-click the \"{}\" playlist under rekordbox xml and choose \
                 Import To Collection.",
                super::rekordbox::PLAYLIST_NAME
            ),
            "Answer Yes to \"Do you want to load information in the tag of the \
             library being imported?\" - that is the step that actually applies \
             the cue points."
                .into(),
            "Check one track you know before importing the whole library: whether \
             an incoming hot cue replaces one already on a track is not documented \
             and is reported inconsistent."
                .into(),
        ],
        ExportTarget::Traktor => vec![
            "Quit Traktor before exporting. It keeps the collection in memory and \
             rewrites collection.nml when it quits, so anything written while it \
             is open is discarded."
                .into(),
            "Keystone edits collection.nml itself, after taking a dated backup you \
             can restore from Settings."
                .into(),
            "Start Traktor afterwards and the cues are on the tracks. No import \
             step is needed."
                .into(),
            "Keystone writes the Key Text field. Traktor's own Key column comes \
             from its analysis and can overwrite what it shows there."
                .into(),
        ],
        ExportTarget::Serato => vec![UNSUPPORTED_SERATO.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serato_reads_as_unsupported_rather_than_merely_unavailable() {
        let s = status(ExportTarget::Serato, &Settings::default(), &[]);
        assert!(!s.supported);
        assert_eq!(s.reason_unavailable.as_deref(), Some(UNSUPPORTED_SERATO));
        assert!(s.detected_path.is_none());
    }

    #[test]
    fn the_rekordbox_copy_describes_an_import_rather_than_a_sync() {
        let steps = manual_steps(ExportTarget::Rekordbox).join(" ");
        assert!(steps.contains("cannot push"));
        assert!(steps.contains("Import To Collection"));
        assert!(steps.contains(super::super::rekordbox::PLAYLIST_NAME));
        assert!(
            !steps.to_lowercase().contains("sync"),
            "the copy must not imply a live sync"
        );
    }

    #[test]
    fn an_override_is_reported_separately_from_detection() {
        let mut settings = Settings::default();
        settings.traktor_path = Some("/somewhere/collection.nml".into());
        let s = status(ExportTarget::Traktor, &settings, &[]);
        assert_eq!(s.override_path.as_deref(), Some("/somewhere/collection.nml"));
    }

    #[test]
    fn an_empty_override_is_not_an_override() {
        let mut settings = Settings::default();
        settings.rekordbox_path = Some(String::new());
        assert!(status(ExportTarget::Rekordbox, &settings, &[])
            .override_path
            .is_none());
    }
}

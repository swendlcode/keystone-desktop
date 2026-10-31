//! The run order, which is the part of this module that matters: refuse, back
//! up, write, verify. The per-target writers have their own tests.

use std::path::Path;

use super::*;
use crate::export::testkit;
use crate::models::Settings;

fn ctx<'a>(backups: &'a Path, settings: &'a Settings) -> ExportContext<'a> {
    ExportContext {
        backups_dir: backups,
        settings,
    }
}

#[test]
fn serato_is_refused_with_the_sentence_the_ui_shows() {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::default();
    let request = testkit::request(ExportTarget::Serato, false);
    for mode in [Mode::Plan, Mode::Write] {
        let err = run(&request, &[], &ctx(dir.path(), &settings), mode).unwrap_err();
        assert!(matches!(err, KeystoneError::ExportRefused(_)), "{err}");
        assert_eq!(err.to_string(), format!("export refused: {}", detect::UNSUPPORTED_SERATO));
    }
}

#[test]
fn the_request_path_beats_the_setting_which_beats_detection() {
    let mut settings = Settings::default();
    settings.rekordbox_path = Some("/from/settings.xml".into());
    let mut request = testkit::request(ExportTarget::Rekordbox, false);
    request.output_path = Some("/from/request.xml".into());
    assert_eq!(
        resolve_destination(&request, &settings).unwrap(),
        Path::new("/from/request.xml")
    );

    request.output_path = None;
    assert_eq!(
        resolve_destination(&request, &settings).unwrap(),
        Path::new("/from/settings.xml")
    );

    // With neither, rekordbox falls back to a suggestion, because the file is
    // ours to put anywhere.
    settings.rekordbox_path = None;
    assert!(resolve_destination(&request, &settings)
        .unwrap()
        .ends_with("keystone-rekordbox.xml"));
}

#[test]
fn traktor_with_nothing_detected_asks_to_be_pointed_at_the_collection() {
    let mut settings = Settings::default();
    settings.traktor_path = None;
    let request = testkit::request(ExportTarget::Traktor, false);
    // Only meaningful on a machine with no Traktor install; where one exists,
    // detection answering is itself the correct outcome.
    match resolve_destination(&request, &settings) {
        Err(e) => assert!(e.to_string().contains("CHANGE"), "{e}"),
        Ok(found) => assert!(found.ends_with("collection.nml")),
    }
}

#[test]
fn a_plan_takes_no_backup_and_a_write_does() {
    let dir = tempfile::tempdir().unwrap();
    let backups = dir.path().join("backups");
    let dest = dir.path().join("keystone-rekordbox.xml");
    std::fs::write(&dest, b"<DJ_PLAYLISTS/>").unwrap();

    let settings = Settings::default();
    let mut request = testkit::request(ExportTarget::Rekordbox, false);
    request.output_path = Some(dest.to_string_lossy().into_owned());

    let plan = dry_run(&request, &[], &ctx(&backups, &settings)).unwrap();
    assert!(plan.backup_path.is_none());
    assert!(!backups.exists());
    assert_eq!(std::fs::read(&dest).unwrap(), b"<DJ_PLAYLISTS/>");

    let report = match export(&request, &[], &ctx(&backups, &settings)) {
        Ok(report) => report,
        // The gate is real: on a machine with rekordbox open this is the
        // correct outcome, not a test failure.
        Err(KeystoneError::ExportRefused(_)) => return,
        Err(e) => panic!("{e}"),
    };
    let backup = report.backup_path.expect("a write backs up what it replaces");
    assert_eq!(std::fs::read(backup).unwrap(), b"<DJ_PLAYLISTS/>");
    // And the replaced file is the new document.
    assert!(std::fs::read_to_string(&dest).unwrap().contains("Keystone"));
}

#[test]
fn every_report_carries_the_manual_steps_for_its_target() {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::default();
    let dest = dir.path().join("out.xml");
    let mut request = testkit::request(ExportTarget::Rekordbox, false);
    request.output_path = Some(dest.to_string_lossy().into_owned());

    let plan = dry_run(&request, &[], &ctx(dir.path(), &settings)).unwrap();
    assert!(!plan.manual_steps.is_empty());
    assert_eq!(plan.target, Some(ExportTarget::Rekordbox));
    assert_eq!(plan.destination.as_deref(), dest.to_str());
    assert_eq!(plan.run_id, request.run_id);
}

#[test]
fn detection_answers_for_all_three_targets() {
    let targets = detect_targets(&Settings::default());
    assert_eq!(targets.len(), 3);
    assert!(targets.iter().all(|t| !t.manual_steps.is_empty()));
    let serato = targets
        .iter()
        .find(|t| t.target == ExportTarget::Serato)
        .unwrap();
    assert!(!serato.supported);
}

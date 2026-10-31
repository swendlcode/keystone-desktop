//! Cue-point export. Three targets, three completely different problems, ranked
//! by difficulty: rekordbox XML < Traktor NML < Serato.
//!
//! The safety layer is built first and is not optional. Everything in
//! [`safety`] exists because of a specific way to destroy an irreplaceable
//! library.
//!
//! This module owns only the shape of a run: resolve a destination, refuse if
//! the DJ app is holding it, take the dated backup, hand off to the target
//! writer, and report. Every target-specific decision lives in its own module
//! so that a format quirk cannot leak into the safety order.

pub mod detect;
pub mod label;
pub mod rekordbox;
pub mod safety;
pub mod serato;
pub mod traktor;

#[cfg(test)]
mod testkit;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{KeystoneError, Result};
use crate::models::{BeatgridMarker, CuePoint, Settings, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportTarget {
    Rekordbox,
    Traktor,
    /// v1.1. The only target that rewrites thousands of irreplaceable audio
    /// files, across four containers, with a testing tail longer than its
    /// implementation.
    Serato,
}

impl ExportTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            ExportTarget::Rekordbox => "rekordbox",
            ExportTarget::Traktor => "traktor",
            ExportTarget::Serato => "serato",
        }
    }

    /// False means the checkbox is disabled with an explanation rather than
    /// present and silently doing nothing.
    pub fn supported_in_v1(self) -> bool {
        !matches!(self, ExportTarget::Serato)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetStatus {
    pub target: ExportTarget,
    pub installed: bool,
    pub detected_path: Option<String>,
    pub override_path: Option<String>,
    /// The sentence the disabled checkbox shows. Never leave a checkbox that
    /// implies something the app cannot do.
    pub reason_unavailable: Option<String>,
    /// True while the DJ app is open. A blocking modal, not a dismissible
    /// warning.
    pub app_running: bool,
    /// v1.1 targets report false so the UI can disable rather than fail late.
    pub supported: bool,
    /// The manual steps the user must take afterwards. rekordbox cannot be
    /// pushed into and Serato needs Rescan ID3 Tags; spell them out on the
    /// export screen or every user files the same ticket.
    pub manual_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub target: ExportTarget,
    pub track_ids: Vec<String>,
    /// Defaults off. The export action shows a count of what would change
    /// before doing anything.
    pub overwrite_existing: bool,
    /// Dry run by default.
    pub dry_run: bool,
    pub output_path: Option<String>,
    /// Groups the `export_log` rows, so an interrupted run across 5,000 tracks
    /// is reportable and resumable rather than silently half-applied.
    pub run_id: String,
}

/// One struct serves both the plan the UI shows before a write and the result it
/// shows after, because the plan is produced by the same code path as the write
/// with the write suppressed. Two structs would let them drift, and a plan that
/// disagrees with the write is worse than no plan.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub run_id: String,
    pub target: Option<ExportTarget>,
    /// Where the bytes went, or would have gone.
    pub destination: Option<String>,
    pub planned: usize,
    pub written: usize,
    pub verified: usize,
    pub skipped: usize,
    pub failed: usize,
    /// Tracks found in the target's collection. Only Traktor can miss: the
    /// rekordbox document is ours and carries every track we are given.
    pub tracks_matched: usize,
    pub tracks_missing: usize,
    pub cues_to_write: usize,
    /// Cues already in the collection that this run would replace. With
    /// `overwrite_existing` off these are left alone and the track is skipped.
    pub cues_to_overwrite: usize,
    pub backup_path: Option<String>,
    pub manual_steps: Vec<String>,
    pub failures: Vec<ExportFailure>,
}

impl ExportReport {
    pub fn for_request(request: &ExportRequest) -> Self {
        Self {
            run_id: request.run_id.clone(),
            target: Some(request.target),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFailure {
    pub track_id: String,
    pub path: String,
    pub reason: String,
}

/// The projection the exporters read. Assembled once by the command so no
/// exporter reaches for a repository.
pub struct ExportTrack {
    pub track: Track,
    pub cues: Vec<CuePoint>,
    pub beatgrid: Vec<BeatgridMarker>,
    /// Pre-formatted under the Key Notation setting, because each target wants
    /// a different field and none of them should re-decide notation.
    pub key_string: String,
}

/// What a run needs that the request cannot carry: the backup root, which is
/// app state rather than something the frontend should be able to name, and the
/// settings the advanced opt-ins live in.
pub struct ExportContext<'a> {
    pub backups_dir: &'a Path,
    pub settings: &'a Settings,
}

/// Detection is best-effort by design: Traktor creates a new root on every
/// version update and the root is user-overridable, so the spec's `CHANGE`
/// button is mandatory.
pub fn detect_targets(settings: &Settings) -> Vec<TargetStatus> {
    detect::all_targets(settings)
}

/// Counts what would change without writing anything.
pub fn dry_run(
    request: &ExportRequest,
    tracks: &[ExportTrack],
    ctx: &ExportContext,
) -> Result<ExportReport> {
    run(request, tracks, ctx, Mode::Plan)
}

pub fn export(
    request: &ExportRequest,
    tracks: &[ExportTrack],
    ctx: &ExportContext,
) -> Result<ExportReport> {
    run(request, tracks, ctx, Mode::Write)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Plan,
    Write,
}

impl Mode {
    pub fn writes(self) -> bool {
        self == Mode::Write
    }
}

/// The fixed order: refuse, back up, write, verify. A target writer is only
/// ever called from here, so none of them can be reached with the gate skipped.
fn run(
    request: &ExportRequest,
    tracks: &[ExportTrack],
    ctx: &ExportContext,
    mode: Mode,
) -> Result<ExportReport> {
    if !request.target.supported_in_v1() {
        return Err(KeystoneError::ExportRefused(
            detect::UNSUPPORTED_SERATO.to_string(),
        ));
    }
    let destination = resolve_destination(request, ctx.settings)?;

    // The gate is on the write path only: a plan touches nothing, and a user
    // with Traktor open still deserves to see what the run would do.
    if mode.writes() {
        safety::assert_not_running(request.target, Some(&destination))?;
    }

    let backup = if mode.writes() && destination.exists() {
        Some(safety::backup_file(
            &destination,
            ctx.backups_dir,
            request.target,
        )?)
    } else {
        None
    };

    let mut report = match request.target {
        ExportTarget::Rekordbox => rekordbox::run(request, tracks, &destination, mode)?,
        ExportTarget::Traktor => traktor::run(request, tracks, &destination, ctx.settings, mode)?,
        ExportTarget::Serato => unreachable!("refused above"),
    };
    report.destination = Some(destination.to_string_lossy().into_owned());
    report.backup_path = backup.map(|p| p.to_string_lossy().into_owned());
    report.manual_steps = detect::manual_steps(request.target);
    Ok(report)
}

/// The request wins, then the user's override, then detection. rekordbox has a
/// suggested default because the file is ours to put anywhere; Traktor has none,
/// because guessing at a path we then rewrite is how cues land in the wrong
/// library.
fn resolve_destination(request: &ExportRequest, settings: &Settings) -> Result<PathBuf> {
    if let Some(p) = request.output_path.as_deref().filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let configured = match request.target {
        ExportTarget::Rekordbox => settings.rekordbox_path.as_deref(),
        ExportTarget::Traktor => settings.traktor_path.as_deref(),
        ExportTarget::Serato => settings.serato_path.as_deref(),
    };
    if let Some(p) = configured.filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    detect::default_destination(request.target).ok_or_else(|| {
        KeystoneError::Export(format!(
            "No {} library was found. Choose it with CHANGE before exporting.",
            request.target.as_str()
        ))
    })
}

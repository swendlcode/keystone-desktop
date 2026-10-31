//! The player's backend.
//!
//! The webview decodes and plays; this side only hands over a path and the peak
//! pyramid the waveform is drawn from. That split is deliberate: the player must
//! never compete with the analysis queue for CPU, and the eight draggable cue
//! markers are database rows, not audio.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::analysis::decode::{self, DecodeOptions};
use crate::core::peaks;
use crate::db::TrackRepository;
use crate::error::{KeystoneError, Result};
use crate::events::{self, WaveformReadyEvent};
use crate::models::{Settings, Track, Waveform};

/// The spec's "Start playing the track 25% of the way in".
const PREVIEW_FRACTION: f64 = 0.25;

/// Resolves the file the webview will load through `convertFileSrc`.
///
/// Checked against the filesystem rather than trusted from the row: an
/// unmounted external drive is the everyday case in a DJ library, and a bare
/// `file://` that silently fails to load reads as a broken play button.
pub fn playback_path(tracks: &Arc<TrackRepository>, track_id: &str) -> Result<String> {
    let track = tracks
        .get_by_id(track_id)?
        .ok_or_else(|| KeystoneError::NotFound(format!("track {track_id}")))?;
    if !Path::new(&track.path).exists() {
        return Err(KeystoneError::NotFound(format!(
            "{} is not reachable — the drive it lives on may not be mounted",
            track.filename
        )));
    }
    Ok(track.path)
}

/// Where playback starts, resolved against the track's own duration so the
/// frontend does not have to know the setting exists.
///
/// An offset at or past the end plays silence and makes the play button look
/// dead, so the fraction is only applied when the duration is known and the
/// result leaves something to play.
pub fn start_offset_ms(track: &Track, settings: &Settings) -> f64 {
    if !settings.start_at_quarter {
        return 0.0;
    }
    let Some(duration) = track.duration_ms.filter(|d| *d > 0) else {
        return 0.0;
    };
    let offset = duration as f64 * PREVIEW_FRACTION;
    if offset >= duration as f64 {
        0.0
    } else {
        offset
    }
}

/// Reads the cached pyramid, or `None` when this track's content has never been
/// decoded for peaks.
///
/// `None` is a normal answer, not an error: the caller schedules the work and
/// the frontend redraws on `WAVEFORM_READY`.
pub fn cached_waveform(peaks_dir: &Path, track: &Track) -> Result<Option<Waveform>> {
    let Some(hash) = track.content_hash.as_deref() else {
        return Ok(None);
    };
    peaks::read(peaks_dir, hash)
}

/// True when the pyramid is already on disk. Lets the caller skip scheduling
/// without paying for the deserialise.
pub fn peaks_cached(peaks_dir: &Path, content_hash: &str) -> bool {
    peaks::cache_path(peaks_dir, content_hash).exists()
}

/// Decodes one file for peaks alone and caches the result.
///
/// Runs on the analysis pool, never on a command's thread: a full decode is
/// seconds of CPU and the one thing the player must not do is take those
/// seconds away from the queue the user is already watching.
pub fn compute_peaks(peaks_dir: &Path, path: &Path, content_hash: &str) -> Result<PathBuf> {
    let opts = DecodeOptions {
        // The key model's 22.05 kHz buffer is the second most expensive thing in
        // a decode and nothing here reads it.
        want_key_rate: false,
        want_peaks: true,
        peak_zoom_levels: peaks::ZOOM_LEVELS.to_vec(),
    };
    let decoded = decode::decode(path, &opts)?;
    let pyramid = decoded
        .peaks
        .ok_or_else(|| KeystoneError::Analysis("the decode returned no peaks".into()))?;
    peaks::write(peaks_dir, content_hash, &pyramid)?;
    Ok(peaks::cache_path(peaks_dir, content_hash))
}

pub fn emit_waveform_ready(app: &AppHandle, track_id: &str, content_hash: &str) {
    let _ = app.emit(
        events::WAVEFORM_READY,
        WaveformReadyEvent {
            track_id: track_id.to_string(),
            content_hash: content_hash.to_string(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Built from JSON rather than a literal so this fixture does not have to
    /// restate fifty columns, and so a new one added to `Track` fails loudly
    /// here rather than silently defaulting.
    fn track(duration_ms: Option<i64>) -> Track {
        let json = format!(
            r#"{{
            "id": "t", "path": "/x.mp3", "filename": "x.mp3", "originalFilename": null,
            "renamePattern": null, "extension": "mp3", "contentHash": null,
            "sizeBytes": null, "durationMs": {duration}, "sampleRate": null,
            "channels": null, "bitDepth": null, "bitrateKbps": null, "codec": null,
            "artist": null, "title": null, "album": null, "genre": null,
            "grouping": null, "comment": null, "year": null, "tagBpm": null,
            "tagKey": null, "hasCoverArt": false,
            "tempoRaw": null, "tempoDisplay": null, "tempoOctaveShift": null,
            "tempoConfidence": null, "tempoResidualMs": null, "tempoVariable": false,
            "keyRoot": null, "keyMode": null, "camelot": null, "keyConfidence": null,
            "keyAltRoot": null, "keyAltMode": null, "keyModeAmbiguous": false,
            "tuningCents": null,
            "energy": null, "energyRaw": null, "peakDb": null, "truePeakDb": null,
            "lufsIntegrated": null, "clippedPeaks": null,
            "rating": null, "dateAdded": 0, "lastPlayed": null, "playCount": 0,
            "keySource": null, "tempoSource": null, "energySource": null,
            "analysisVersion": 0, "energyModelVersion": 0, "analyzedAt": null,
            "indexStatus": "indexed", "errorMessage": null,
            "createdAt": 0, "updatedAt": 0, "lastSeenAt": 0
        }}"#,
            duration = match duration_ms {
                Some(d) => d.to_string(),
                None => "null".to_string(),
            }
        );
        serde_json::from_str(&json).expect("the test fixture must match Track")
    }

    #[test]
    fn the_preview_offset_is_off_by_default() {
        let settings = Settings::default();
        assert!(!settings.start_at_quarter);
        assert_eq!(start_offset_ms(&track(Some(240_000)), &settings), 0.0);
    }

    #[test]
    fn an_unknown_duration_starts_from_zero_rather_than_guessing() {
        let settings = Settings {
            start_at_quarter: true,
            ..Settings::default()
        };
        assert_eq!(start_offset_ms(&track(None), &settings), 0.0);
        assert_eq!(start_offset_ms(&track(Some(0)), &settings), 0.0);
        assert_eq!(start_offset_ms(&track(Some(240_000)), &settings), 60_000.0);
    }
}

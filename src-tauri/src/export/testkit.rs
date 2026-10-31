//! Fixture builders for the exporter tests.
//!
//! `Track` is a 50-field row and every exporter test needs one, so the shape is
//! written once here. It is built from JSON rather than a struct literal for
//! two reasons: the literal is one expression far past the function-length cap
//! and cannot be split, and deserialising means a fixture that drifts from the
//! real `Track` fails loudly in every test rather than being quietly updated.
//!
//! Test-only: never compiled into a shipped binary.

use serde_json::{json, Map, Value};

use crate::export::{ExportRequest, ExportTarget, ExportTrack};
use crate::models::{BeatgridMarker, CueKind, CuePoint, CueSource, Track};

pub fn track(path: &str) -> Track {
    let filename = path.rsplit('/').next().unwrap_or(path).to_string();
    let mut fields = Map::new();
    for group in [identity(path, &filename), tags(), analysis()] {
        if let Value::Object(group) = group {
            fields.extend(group);
        }
    }
    serde_json::from_value(Value::Object(fields))
        .expect("the fixture no longer matches the Track shape")
}

fn identity(path: &str, filename: &str) -> Value {
    json!({
        "id": format!("id-{filename}"),
        "path": path,
        "filename": filename,
        "originalFilename": null,
        "renamePattern": null,
        "extension": "mp3",
        "contentHash": "hash",
        "sizeBytes": 8_123_456,
        "durationMs": 192_078,
        "sampleRate": 44_100,
        "channels": 2,
        "bitDepth": null,
        "bitrateKbps": 320,
        "codec": "mp3",
        "dateAdded": 1_762_000_000,
        "lastPlayed": null,
        "playCount": 4,
        "createdAt": 1_762_000_000,
        "updatedAt": 1_762_000_100,
        "lastSeenAt": 1_762_000_100,
        "indexStatus": "indexed",
        "errorMessage": null
    })
}

fn tags() -> Value {
    json!({
        "artist": "Swendl",
        "title": "Night Shift",
        "album": "Keystone EP",
        "genre": "House",
        "grouping": null,
        "comment": null,
        "year": 2026,
        "tagBpm": null,
        "tagKey": null,
        "hasCoverArt": false,
        "rating": 3
    })
}

fn analysis() -> Value {
    json!({
        "tempoRaw": 128.0,
        "tempoDisplay": 128.0,
        "tempoOctaveShift": 0,
        "tempoConfidence": 0.95,
        "tempoResidualMs": 6.0,
        "tempoVariable": false,
        "keyRoot": 9,
        "keyMode": "minor",
        "camelot": "8A",
        "keyConfidence": 0.88,
        "keyAltRoot": null,
        "keyAltMode": null,
        "keyModeAmbiguous": false,
        "tuningCents": 0.0,
        "energy": 6,
        "energyRaw": 0.61,
        "peakDb": -0.3,
        "truePeakDb": -0.1,
        "lufsIntegrated": -8.2,
        "clippedPeaks": 0,
        "keySource": "detected",
        "tempoSource": "detected",
        "energySource": "detected",
        "analysisVersion": 1,
        "energyModelVersion": 1,
        "analyzedAt": 1_762_000_100
    })
}

pub fn cue(slot: u8, position_ms: f64, kind: CueKind) -> CuePoint {
    CuePoint {
        id: format!("cue-{slot}"),
        track_id: "track".into(),
        slot,
        kind,
        position_ms,
        end_ms: None,
        bar_index: Some(0),
        phrase_level: Some(16),
        label: None,
        color: Some("#99F23F".into()),
        confidence: Some(0.8),
        source: CueSource::Detected,
        created_at: 1_762_000_100,
    }
}

pub fn grid(position_ms: f64, bpm: f64) -> BeatgridMarker {
    BeatgridMarker {
        id: "grid".into(),
        track_id: "track".into(),
        position_ms,
        bpm,
        beats_per_bar: 4,
        beat_in_bar: 1,
    }
}

/// A track with a grid and two cues, which is the ordinary case both exporters
/// have to get right.
pub fn export_track(path: &str) -> ExportTrack {
    ExportTrack {
        track: track(path),
        cues: vec![
            cue(0, 15_234.0, CueKind::Intro),
            cue(1, 64_000.0, CueKind::Drop),
        ],
        beatgrid: vec![grid(123.456, 128.0)],
        key_string: "8A".into(),
    }
}

pub fn request(target: ExportTarget, overwrite: bool) -> ExportRequest {
    ExportRequest {
        target,
        track_ids: vec!["id-track.mp3".into()],
        overwrite_existing: overwrite,
        dry_run: false,
        output_path: None,
        run_id: "run-1".into(),
    }
}

//! Shared test fixtures for the tag and rename modules.
//!
//! `Track` has sixty fields and no `Default`, so the fixture is built from JSON
//! rather than a struct literal repeated in every test file. The `expect` is
//! load-bearing: if a field is added or renamed, the fixture fails loudly here
//! instead of quietly deserialising into something else.

use crate::models::{KeyMode, Track};

pub fn track() -> Track {
    serde_json::from_str(
        r#"{"id":"t","path":"/music/Midnight.mp3","filename":"Midnight.mp3",
        "originalFilename":null,"renamePattern":null,"extension":"mp3",
        "contentHash":null,"sizeBytes":null,"durationMs":null,"sampleRate":null,
        "channels":null,"bitDepth":null,"bitrateKbps":null,"codec":null,
        "artist":null,"title":null,"album":null,"genre":null,"grouping":null,
        "comment":null,"year":null,"tagBpm":null,"tagKey":null,"hasCoverArt":false,
        "tempoRaw":null,"tempoDisplay":null,"tempoOctaveShift":null,
        "tempoConfidence":null,"tempoResidualMs":null,"tempoVariable":false,
        "keyRoot":null,"keyMode":null,"camelot":null,"keyConfidence":null,
        "keyAltRoot":null,"keyAltMode":null,"keyModeAmbiguous":false,
        "tuningCents":null,"energy":null,"energyRaw":null,"peakDb":null,
        "truePeakDb":null,"lufsIntegrated":null,"clippedPeaks":null,"rating":null,
        "dateAdded":0,"lastPlayed":null,"playCount":0,"keySource":null,
        "tempoSource":null,"energySource":null,"analysisVersion":0,
        "energyModelVersion":0,"analyzedAt":null,"indexStatus":"indexed",
        "errorMessage":null,"createdAt":0,"updatedAt":0,"lastSeenAt":0}"#,
    )
    .expect("the fixture must match the Track struct")
}

/// A analysed at A minor — 8A on the wheel — 126.4 BPM, energy 7.
pub fn analysed() -> Track {
    let mut t = track();
    t.key_root = Some(9);
    t.key_mode = Some(KeyMode::Minor);
    t.energy = Some(7);
    t.tempo_display = Some(126.4);
    t
}

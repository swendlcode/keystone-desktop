//! S1 — the one shared decode pass.
//!
//! This module is meant to be lifted into a crate shared with Capstone, so it
//! must not reach for anything Keystone-specific beyond the error type. The
//! symphonia pin is 0.5 on purpose: the proven streaming-decode shape is a 0.5
//! call site and 0.6 redesigned the audio primitives. The port is its own task,
//! paid for once, in the shared crate.

mod levels;
mod open;
mod rate;
mod stream;

// Containers written by the tests themselves. Shared with the dev-only eval
// feature for the same reason the DSP generators are: the harness has to build
// its files from the *same* writer the decode tests use, or a writer bug can
// flatter one and hide from the other.
#[cfg(any(test, feature = "eval"))]
pub mod fixtures;
#[cfg(test)]
mod tests;

use std::path::Path;

use crate::core::peaks::PeakPyramid;
use crate::error::Result;
use crate::models::LevelsResult;

/// The key model's input rate. Fixed by the graph, not a choice.
pub const KEY_SAMPLE_RATE: u32 = 22_050;
/// Everything else runs at the analysis rate.
pub const ANALYSIS_SAMPLE_RATE: u32 = 44_100;

/// Header-only facts. Cheap enough to run during the walk, so the table can
/// show duration and bitrate before anything is analysed.
#[derive(Debug, Clone)]
pub struct AudioProbe {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: Option<u16>,
    pub bitrate_kbps: Option<u32>,
    pub duration_ms: Option<i64>,
    pub n_frames: Option<u64>,
}

/// Where the audio actually starts and ends inside the file.
///
/// This exists so the content hash can skip the tag block. ID3 sits at the head
/// of an MP3, so hashing the first bytes of the *file* means Keystone's own
/// re-tag pass changes every hash and silently wipes its own analysis cache. It
/// looks like it works right up until the first re-tag.
#[derive(Debug, Clone)]
pub struct AudioRegion {
    pub codec: String,
    pub sample_rate: u32,
    pub n_frames: u64,
    /// Byte offset of the first audio packet.
    pub data_offset: u64,
    pub data_len: u64,
}

#[derive(Debug, Clone)]
pub struct DecodeOptions {
    /// Skip the 22.05 kHz buffer when the key stage will not run.
    pub want_key_rate: bool,
    /// Accumulate waveform peaks inside the decode loop rather than over a
    /// retained buffer, which keeps it O(target buckets) in memory.
    pub want_peaks: bool,
    pub peak_zoom_levels: Vec<u32>,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            want_key_rate: true,
            want_peaks: true,
            peak_zoom_levels: crate::core::peaks::ZOOM_LEVELS.to_vec(),
        }
    }
}

pub struct DecodedAudio {
    /// Mono f32 at 44,100 Hz, arithmetic-mean downmix.
    pub mono_44100: Vec<f32>,
    /// Mono f32 at 22,050 Hz. Present only when `want_key_rate` was set.
    pub mono_22050: Option<Vec<f32>>,
    pub probe: AudioProbe,
    pub duration_ms: i64,
    pub levels: LevelsResult,
    pub peaks: Option<PeakPyramid>,
}

pub fn probe(path: &Path) -> Result<AudioProbe> {
    open::probe(path)
}

pub fn audio_region(path: &Path) -> Result<AudioRegion> {
    open::audio_region(path)
}

/// Decodes once: arithmetic-mean downmix, rubato for the rate conversion, true
/// peak and clipped-sample counting in the same pass, and streaming peak
/// accumulation. Never retain a decoded track beyond this call.
pub fn decode(path: &Path, opts: &DecodeOptions) -> Result<DecodedAudio> {
    stream::decode(path, opts)
}

/// Rate conversion for the stages that need it, and for the tuning correction.
pub fn resample(samples: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    rate::resample(samples, from, to)
}

pub fn is_supported_extension(ext: &str) -> bool {
    crate::core::scanner::AUDIO_EXTENSIONS.contains(&ext)
}

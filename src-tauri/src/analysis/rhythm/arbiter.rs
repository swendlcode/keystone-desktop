//! The seam a neural downbeat tracker slots into, and nothing more.
//!
//! **Beat This! is behind a gate, not in the build by default.** It is MIT for
//! code *and* weights, which makes it the only permissively licensed path to a
//! neural downbeat at all — madmom's models are CC BY-NC-SA and unusable. But
//! the fp32 ONNX export is 190 MB and it measured 68x realtime at best, with
//! the CoreML execution provider 3x *slower* than CPU. A 190 MB bundled model
//! is not distributable and a download on first run contradicts the offline
//! premise, so nothing is bundled until the measurement in ENGINE.md §2 says
//! the classical phase needs help: 200 EDM tracks, and if the classical
//! downbeat agrees with the model on more than 90% at +-70 ms, the model is
//! dropped and 190 MB disappears from the problem.
//!
//! The seam is a trait over times, not over a model. `beats::analyze_with_
//! arbiter` takes downbeat times, so this module is the only place that would
//! ever see audio or an ONNX session, and `beats.rs` needs no change on the day
//! one arrives.

use crate::error::Result;

/// Anything that can propose downbeat times for a decoded track.
pub trait DownbeatArbiter: Send + Sync {
    /// Downbeat times in ms. An arbiter that cannot read this track returns an
    /// empty vector rather than an error: a missing opinion is not a failed
    /// analysis, and the classical phase is always available.
    fn downbeats_ms(&self, samples: &[f32], sample_rate: u32) -> Result<Vec<f64>>;
}

/// Asks an arbiter for downbeats, treating a failure as no opinion.
///
/// The classical phase is always available, so an arbiter that cannot read this
/// track must not fail the analysis — which is the whole reason this wrapper
/// exists rather than callers using the trait directly.
pub fn downbeats_or_none(arbiter: &dyn DownbeatArbiter, samples: &[f32], sample_rate: u32) -> Vec<f64> {
    arbiter.downbeats_ms(samples, sample_rate).unwrap_or_default()
}

/// Beat This! (CPJKU). Compiled only under the `beat-this` feature, because the
/// feature is what carries the model asset.
#[cfg(feature = "beat-this")]
pub mod beat_this {
    use super::*;

    /// From the shipped config, and all of it load-bearing: 22,050 Hz mono,
    /// `n_fft` 1024, hop 441 -> 50 fps, **periodic** Hann, `center=True`
    /// reflect pad, magnitude / sqrt(n_fft), 128 mel, `log1p(1000x)`,
    /// 1500-frame windows, border 6, logits > 0, peak kernel 7, threshold 0.0,
    /// dedupe width 1, each downbeat snapped to its nearest detected beat.
    pub const SAMPLE_RATE: u32 = 22_050;
    pub const N_FFT: usize = 1024;
    pub const HOP: usize = 441;
    pub const N_MELS: usize = 128;
    pub const WINDOW_FRAMES: usize = 1500;
    pub const BORDER_SIZE: usize = 6;

    /// Where the sampled-window strategy takes its windows from, as a fraction
    /// of the core region. Three 30 s windows is ~1.3 s per track against 5.3 s
    /// for full-track inference.
    pub const SAMPLE_POSITIONS: [f64; 3] = [0.25, 0.50, 0.75];

    pub struct BeatThisSession {
        #[allow(dead_code)]
        model_path: std::path::PathBuf,
    }

    impl BeatThisSession {
        pub fn from_path(path: &std::path::Path) -> Result<Self> {
            Err(crate::error::KeystoneError::Analysis(format!(
                "Beat This! is not bundled: {} is behind the measurement gate in \
                 ENGINE.md §2 and the model must be int8-quantised before it ships",
                path.display()
            )))
        }

        /// One window at a time, because the published export is **batch-1
        /// only** — a `[12, 1500, 128]` input fails with a Reshape error.
        pub fn infer_window(&self, _samples: &[f32]) -> Result<(Vec<f64>, Vec<f64>)> {
            Err(crate::error::KeystoneError::Analysis(
                "Beat This! inference is not implemented".into(),
            ))
        }
    }

    impl DownbeatArbiter for BeatThisSession {
        fn downbeats_ms(&self, _samples: &[f32], _sample_rate: u32) -> Result<Vec<f64>> {
            Ok(Vec::new())
        }
    }
}

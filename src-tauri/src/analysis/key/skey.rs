//! S-KEY via ONNX. VQT, dB scaling, cropping and ChromaNet are all inside the
//! graph, so there is no DSP to reimplement here — only the chunking, the label
//! order and the probability averaging.

use std::path::Path;

use crate::error::Result;
use crate::models::KeyMode;

/// Fixed by the graph.
pub const SAMPLE_RATE: u32 = 22_050;

/// Chunk and average the probability vectors rather than feeding the whole
/// file. Whole-file inference gave top-1 0.268 on the one test track against
/// 0.480 for chunk averaging, and per-chunk accuracy rose with window length
/// (3 s 30%, 5 s 50%, 10 s 67%, 15 s 100%). That accuracy figure is n=1 and is
/// a hypothesis for the eval harness; the confidence lift is real, and chunking
/// yields per-region key for free, which the deferred deck view needs.
pub const CHUNK_SECONDS: f32 = 15.0;

/// Below this the model has nothing to work with.
pub const MIN_SECONDS: f32 = 3.0;

/// 24 already-softmaxed scores.
#[derive(Debug, Clone, Copy)]
pub struct KeyProbabilities {
    /// Label order is **12 majors starting at A, then 12 minors starting at B**.
    /// G major is 10 and A minor is 22. Getting this wrong transposes every
    /// result and no unit test on the DSP would catch it, which is why
    /// [`label_to_key`] is tested against both anchors.
    pub p: [f32; 24],
}

impl KeyProbabilities {
    pub fn zeroed() -> Self {
        Self { p: [0.0; 24] }
    }

    /// Top label and its probability.
    pub fn best(&self) -> (usize, f32) {
        let mut best = 0usize;
        for i in 1..24 {
            if self.p[i] > self.p[best] {
                best = i;
            }
        }
        (best, self.p[best])
    }

    /// Second-best label, stored as the runner-up key on the track row.
    pub fn runner_up(&self) -> (usize, f32) {
        let (best, _) = self.best();
        let mut second = (best + 1) % 24;
        for i in 0..24 {
            if i != best && self.p[i] > self.p[second] {
                second = i;
            }
        }
        (second, self.p[second])
    }
}

/// Label index to (pitch class with 0 = C, mode).
pub fn label_to_key(index: usize) -> (u8, KeyMode) {
    // A is pitch class 9; B is 11.
    if index < 12 {
        (((9 + index) % 12) as u8, KeyMode::Major)
    } else {
        (((11 + (index - 12)) % 12) as u8, KeyMode::Minor)
    }
}

pub fn key_to_label(root: u8, mode: KeyMode) -> usize {
    let p = (root % 12) as i32;
    match mode {
        KeyMode::Major => ((p - 9).rem_euclid(12)) as usize,
        KeyMode::Minor => (((p - 11).rem_euclid(12)) + 12) as usize,
    }
}

/// A loaded session. Built once and shared across workers: an ONNX session is
/// expensive to construct and cheap to call.
pub struct SkeySession {
    #[allow(dead_code)]
    inner: (),
}

impl SkeySession {
    /// The model is bundled, 324,536 bytes, opset 17, fp32. Nothing in the
    /// analysis path may download it: if a model cannot be bundled, it does not
    /// ship.
    pub fn from_bytes(_model: &[u8]) -> Result<Self> {
        todo!("owes: build an ort Session from the bundled graph")
    }

    pub fn from_path(_path: &Path) -> Result<Self> {
        todo!("owes: build an ort Session from a file, for the eval harness")
    }

    /// One chunk: peak-normalise to 1.0, feed `[1, N]` f32 at 22,050 Hz, read
    /// `[1, 24]`. Peak normalisation is part of the contract, not a nicety.
    pub fn infer_chunk(&self, _samples: &[f32]) -> Result<KeyProbabilities> {
        todo!("owes: peak-normalise, run the session, return the 24 scores")
    }

    /// Chunks at 15 s, averages the probability vectors, and returns the
    /// per-chunk results alongside so S4 can store them as `chunk_keys`.
    pub fn infer(&self, _samples: &[f32]) -> Result<(KeyProbabilities, Vec<KeyProbabilities>)> {
        todo!("owes: 15 s chunking and probability averaging")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_order_matches_the_two_published_anchors() {
        // The whole model integration rests on these two.
        assert_eq!(label_to_key(10), (7, KeyMode::Major)); // G major
        assert_eq!(label_to_key(22), (9, KeyMode::Minor)); // A minor
        assert_eq!(label_to_key(0), (9, KeyMode::Major)); // majors start at A
        assert_eq!(label_to_key(12), (11, KeyMode::Minor)); // minors start at B
    }

    #[test]
    fn label_mapping_round_trips_over_all_24() {
        for i in 0..24 {
            let (root, mode) = label_to_key(i);
            assert_eq!(key_to_label(root, mode), i);
        }
    }
}

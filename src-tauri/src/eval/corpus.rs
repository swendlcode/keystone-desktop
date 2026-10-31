//! A synthetic key corpus with known ground truth, generated in process.
//!
//! **It is not GiantSteps and its numbers do not transfer.** Synthetic chords
//! have no production, no percussion over the harmony, no sampled material and
//! no annotator disagreement, so a score here is an upper bound on a corpus that
//! is deliberately easier than the real one. What it is good for is the thing a
//! real corpus cannot do on this machine: it depends on no files, it states its
//! own answer, and it says by construction which items have **no** right answer
//! at all. Nothing published may come from it; see [`super::report`], which
//! prints that caveat next to every figure.
//!
//! The generators are `analysis::spectral::test_signals` and
//! `test_unpitched` — the same ones the stage's own unit tests use, on purpose.
//! A corpus with its own copy of a chord renderer can pass while the renderer
//! and the detector drift together.

use crate::analysis::spectral::{test_signals as sig, test_unpitched as unp};
use crate::models::KeyMode;

/// 22,050 Hz. The key model's own rate, and half the analysis rate, which gives
/// the low end better resolution per FFT point rather than worse — nothing here
/// is made artificially easy by it.
pub const SAMPLE_RATE: u32 = sig::TEST_RATE;

/// Long enough for two 15 s chunk votes to be a different question from one, and
/// short enough that the whole corpus runs inside a test. Chunk voting below
/// one chunk length falls back to the whole-track PCP, which is a path worth
/// covering too, so the unpitched items are longer.
pub const SECONDS: f32 = 8.0;

/// Detunes the corpus is rendered at, in cents, plus the in-tune pass.
///
/// Both are inside Faraldo's ±1/3-semitone clamp and far enough outside the
/// 10-cent skip threshold to force the corrective resample. A turntable at
/// −35 cents is a real thing; so is a sampled loop that was pitched to fit.
pub const DETUNE_CENTS: [f32; 2] = [-35.0, 22.0];

/// What the corpus says the answer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truth {
    Key { root: u8, mode: KeyMode },
    /// The material supports no key. The only right answer is to refuse, so
    /// these items are scored as refusals and never folded into a MIREX figure:
    /// giving them a reference key would mean scoring an honest "don't know"
    /// as an error.
    NoKey,
}

#[derive(Debug, Clone, Copy)]
enum Kind {
    MajorPop,
    MajorCadence,
    MinorIv,
    Percussion,
    ChromaticCluster,
    Silence,
}

/// A corpus item before it is rendered. Descriptors are cheap; the audio is
/// not, so the harness renders one at a time and drops it.
#[derive(Debug, Clone)]
pub struct Spec {
    pub name: String,
    kind: Kind,
    pub truth: Truth,
    pub detune_cents: f32,
    pub seconds: f32,
    pub sample_rate: u32,
}

pub struct Item {
    pub spec: Spec,
    pub samples: Vec<f32>,
}

impl Spec {
    pub fn render(&self) -> Item {
        let samples = match self.kind {
            Kind::MajorPop => self.progression(sig::major_pop()),
            Kind::MajorCadence => self.progression(sig::major_cadence()),
            Kind::MinorIv => self.progression(sig::minor_iv()),
            Kind::Percussion => unp::percussion(self.sample_rate, self.seconds),
            Kind::ChromaticCluster => unp::chromatic_cluster(self.sample_rate, self.seconds),
            Kind::Silence => unp::silence(self.sample_rate, self.seconds),
        };
        Item { spec: self.clone(), samples }
    }

    fn progression(&self, chords: Vec<sig::Chord>) -> Vec<f32> {
        let tonic = match self.truth {
            Truth::Key { root, .. } => root as i32,
            Truth::NoKey => 0,
        };
        sig::progression(&chords, tonic, self.sample_rate, self.seconds, self.detune_cents)
    }

    pub fn reference_key(&self) -> Option<(u8, KeyMode)> {
        match self.truth {
            Truth::Key { root, mode } => Some((root, mode)),
            Truth::NoKey => None,
        }
    }
}

const PITCH_NAMES: [&str; 12] =
    ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

/// The 24 keys, each as three progressions: two major shapes over the twelve
/// major keys and one minor shape over the twelve minor ones.
///
/// Both major shapes are kept because they test different things. `I-V-vi-IV-I`
/// has a doubled tonic and is what lets a profile match separate a key from its
/// relative at all; `I-IV-V7-I` puts the leading tone in through the dominant
/// seventh, which is the evidence a real cadence gives and a loop does not.
pub fn keyed(detune_cents: f32) -> Vec<Spec> {
    let mut out = Vec::with_capacity(36);
    for root in 0..12u8 {
        for (kind, mode, label) in [
            (Kind::MajorPop, KeyMode::Major, "I-V-vi-IV-I"),
            (Kind::MajorCadence, KeyMode::Major, "I-IV-V7-I"),
            (Kind::MinorIv, KeyMode::Minor, "i-iv-i-bVII-bVI"),
        ] {
            let suffix = if detune_cents == 0.0 {
                String::new()
            } else {
                format!(" @{detune_cents:+.0}c")
            };
            out.push(Spec {
                name: format!(
                    "{} {} {label}{suffix}",
                    PITCH_NAMES[root as usize],
                    mode.as_str(),
                ),
                kind,
                truth: Truth::Key { root, mode },
                detune_cents,
                seconds: SECONDS,
                sample_rate: SAMPLE_RATE,
            });
        }
    }
    out
}

/// Material with no key: a drum loop, a twelve-note cluster and silence.
///
/// All three are the same requirement from different directions. Percussion is
/// "no pitch at all"; the cluster is "pitch everywhere", which is the harder
/// case because the chroma is full rather than empty and a stage that only
/// checks for silence reads it as a key; silence is the degenerate input a
/// failed decode hands the worker.
///
/// Longer than the keyed items because a refusal has to survive a track's worth
/// of chunk votes, not one.
pub fn unpitched() -> Vec<Spec> {
    [
        (Kind::Percussion, "drum loop", 20.0f32),
        (Kind::ChromaticCluster, "chromatic cluster", 16.0),
        (Kind::Silence, "silence", 16.0),
        (Kind::Silence, "silence, half a second", 0.5),
    ]
    .into_iter()
    .map(|(kind, name, seconds)| Spec {
        name: name.to_string(),
        kind,
        truth: Truth::NoKey,
        detune_cents: 0.0,
        seconds,
        sample_rate: SAMPLE_RATE,
    })
    .collect()
}

/// Every keyed item at every detune, in one list.
pub fn detuned() -> Vec<Spec> {
    DETUNE_CENTS.iter().flat_map(|c| keyed(*c)).collect()
}

/// The whole corpus: the 24 keys in tune, the same detuned, and the material
/// that must come back refused.
pub fn full() -> Vec<Spec> {
    let mut out = keyed(0.0);
    out.extend(detuned());
    out.extend(unpitched());
    out
}

/// Arithmetic-mean downmix of an interleaved stereo buffer.
///
/// S1 owns the real one and S1 is a stub, so this is the harness's own copy of
/// the rule ENGINE.md states — mean, not sum, and not left-only. It exists for
/// one check: a stereo source reduced this way has to analyse to the same key as
/// the mono original, because every stage after S1 only ever sees one channel
/// and a downmix bug would look like a detector bug.
pub fn downmix_stereo(interleaved: &[f32]) -> Vec<f32> {
    interleaved.chunks(2).map(|f| f.iter().sum::<f32>() / f.len() as f32).collect()
}

/// Interleaves two channels, for building a stereo source to downmix.
pub fn interleave(left: &[f32], right: &[f32]) -> Vec<f32> {
    let n = left.len().min(right.len());
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        out.push(left[i]);
        out.push(right[i]);
    }
    out
}

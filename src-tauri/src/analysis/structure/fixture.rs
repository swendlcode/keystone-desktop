//! A synthetic track, built in the spectral domain, for the energy and cue
//! tests.
//!
//! It exists because §7.3's two free tests have to run against the real feature
//! and scoring code rather than against a mock, and the two expensive shared
//! transforms (the STFT and the HPSS median filters) belong to
//! `analysis::frontend`. Building the magnitude, harmonic and percussive
//! matrices directly through [`Spectra::from_parts`] enters the pipeline exactly
//! where S9 does, so every line of S9, S10 and S11 under test is the shipped one.
//!
//! The perturbations the invariance test needs are applied here as what they do
//! to a spectrum: a gain change is a multiplier, a wide-Q EQ tilt is a smooth
//! per-bin gain, and a sample-rate change moves every partial into a different
//! bin and every frame onto a different grid. Running the real encoders and
//! resamplers over fifty real tracks is the eval harness's job; what matters on
//! day one is that no feature is allowed to move when only the level does.

mod render;

use ndarray::Array2;

use crate::analysis::beats::BeatAnalysis;
use crate::analysis::energy::loudness::ShortTerm;
use crate::analysis::structure::spectra::{Spectra, HOP};

use self::render::{apply_codec, apply_shaping, grid_for, render_section, short_term_from};

/// Top of the modelled spectrum. Covers every band the features read — the hat
/// band starts at 6 kHz — while keeping the matrices small enough for a unit
/// test.
pub const TOP_HZ: f32 = 11_025.0;

/// Bins needed to reach [`TOP_HZ`] at this rate, capped at the real transform's
/// bin count.
///
/// A **frequency** ceiling rather than a bin count, which matters for exactly
/// one test: with a fixed 1,024 bins the modelled spectrum ran to 11 kHz at
/// 44.1 kHz but only 8 kHz at 32 kHz, so re-rendering at another rate truncated
/// the hat band and the resample case failed by 2 energy steps — a property of
/// the fixture, not of resampling. Pinning the ceiling in hertz is what makes
/// the resample case a test of partials landing in different bins and frames
/// landing on a different grid, which is what a resample actually does.
pub fn n_bins(sample_rate: u32) -> usize {
    let bin_hz = sample_rate as f32 / crate::analysis::structure::spectra::N_FFT as f32;
    ((TOP_HZ / bin_hz).ceil() as usize).clamp(1, crate::analysis::structure::spectra::N_FFT / 2 + 1)
}

/// One section of the synthetic arrangement. Levels are linear magnitudes.
#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub bars: usize,
    pub kick: f32,
    pub snare: f32,
    pub hat: f32,
    pub harm: f32,
    /// Ramp every percussive level from a quarter of its value to its full
    /// value across the section, which is what a build is.
    pub ramp: bool,
}

impl Section {
    pub const fn new(bars: usize, kick: f32, snare: f32, hat: f32, harm: f32) -> Self {
        Self {
            bars,
            kick,
            snare,
            hat,
            harm,
            ramp: false,
        }
    }

    pub const fn ramped(mut self) -> Self {
        self.ramp = true;
        self
    }
}

/// What a lossy codec does to a magnitude spectrum, as a surrogate.
///
/// Three effects, and the first is the one that does nothing here: the
/// band-limit sits at the encoder's cutoff, which is above everything this
/// fixture renders (nothing above 9 kHz), so on this material it removes no
/// modelled content. What it does exercise is the part that can actually move
/// the features — bins below the masking threshold being discarded, and a
/// quantisation noise floor arriving in their place. `flux_mean`, `high_ratio`
/// and `perc_ratio` are all reachable from those two.
///
/// **This is a surrogate, not a round trip.** The real test is 50 real tracks
/// through a real encoder at 320/192/128 and back, from ENGINE.md 7.3 test 1,
/// and it needs an encoder and a corpus. Saying the surrogate passed is not
/// saying that test passed.
#[derive(Debug, Clone, Copy)]
pub struct Codec {
    pub cutoff_hz: f32,
    /// Bins under this fraction of the peak magnitude are discarded.
    pub hole_floor: f32,
    /// Noise added, as a fraction of the peak magnitude.
    pub noise_floor: f32,
}

/// How a rendered fixture differs from the reference render.
#[derive(Debug, Clone, Copy)]
pub struct Perturbation {
    /// Linear magnitude multiplier. `10^(dB/20)`.
    pub gain: f32,
    /// Wide-Q tilt in dB per decade of frequency.
    pub tilt_db: f32,
    pub sample_rate: u32,
    pub codec: Option<Codec>,
}

impl Default for Perturbation {
    fn default() -> Self {
        Self {
            gain: 1.0,
            tilt_db: 0.0,
            sample_rate: 44_100,
            codec: None,
        }
    }
}

impl Perturbation {
    pub fn gain_db(db: f32) -> Self {
        Self {
            gain: 10f32.powf(db / 20.0),
            ..Self::default()
        }
    }

    pub fn tilt(db: f32) -> Self {
        Self {
            tilt_db: db,
            ..Self::default()
        }
    }

    pub fn resampled(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            ..Self::default()
        }
    }

    /// A lossy re-encode at `kbps`, as the three spectral effects in [`Codec`].
    ///
    /// The numbers are the published behaviour of a mainstream MP3 encoder:
    /// roughly 16 kHz of bandwidth at 128 kbps, 19 at 192 and 20.5 at 320, with
    /// the quantisation floor and the holes it punches getting coarser as the
    /// bit budget shrinks.
    pub fn reencode(kbps: u32) -> Self {
        let codec = match kbps {
            0..=128 => Codec { cutoff_hz: 16_000.0, hole_floor: 0.020, noise_floor: 0.0040 },
            129..=192 => Codec { cutoff_hz: 19_000.0, hole_floor: 0.006, noise_floor: 0.0015 },
            _ => Codec { cutoff_hz: 20_500.0, hole_floor: 0.002, noise_floor: 0.0005 },
        };
        Self {
            codec: Some(codec),
            ..Self::default()
        }
    }
}

pub struct Fixture {
    pub spectra: Spectra,
    pub beats: BeatAnalysis,
    pub short_term: ShortTerm,
    pub duration_ms: i64,
    /// Section spans in ms, in arrangement order, so a test can ask for one
    /// section's energy without depending on S9 agreeing with it.
    pub sections: Vec<(f64, f64)>,
}

/// Renders the arrangement. `bpm` fixes the grid; `p` says how this render
/// differs from the reference one.
pub fn render(sections: &[Section], bpm: f64, p: Perturbation) -> Fixture {
    let beat_ms = 60_000.0 / bpm;
    let bar_ms = beat_ms * 4.0;
    let total_bars: usize = sections.iter().map(|s| s.bars).sum();
    let duration_ms = (total_bars as f64 * bar_ms) as i64;

    let fps = p.sample_rate as f64 / HOP as f64;
    let n_frames = ((duration_ms as f64 / 1000.0) * fps).floor() as usize;
    let bin_hz = p.sample_rate as f32 / crate::analysis::structure::spectra::N_FFT as f32;

    let n_bins = n_bins(p.sample_rate);
    let mut harmonic = Array2::<f32>::zeros((n_frames, n_bins));
    let mut percussive = Array2::<f32>::zeros((n_frames, n_bins));
    let mut spans = Vec::with_capacity(sections.len());

    let mut bar = 0usize;
    for section in sections {
        let start_ms = bar as f64 * bar_ms;
        let end_ms = (bar + section.bars) as f64 * bar_ms;
        spans.push((start_ms, end_ms));
        render_section(
            &mut harmonic,
            &mut percussive,
            section,
            bar,
            bar_ms,
            beat_ms,
            fps,
            bin_hz,
        );
        bar += section.bars;
    }

    apply_shaping(&mut harmonic, bin_hz, p);
    apply_shaping(&mut percussive, bin_hz, p);
    if let Some(codec) = p.codec {
        // Both components, with different noise seeds: the codec acts on the
        // mix, and a surrogate that noised only one of them would hand the HPSS
        // features a result no encoder produces.
        apply_codec(&mut harmonic, bin_hz, codec, 0x51A7_C0DE_0000_0001);
        apply_codec(&mut percussive, bin_hz, codec, 0x51A7_C0DE_0000_0002);
    }
    let magnitude = &harmonic + &percussive;
    let short_term = short_term_from(&magnitude, fps);

    Fixture {
        beats: grid_for(duration_ms, beat_ms),
        spectra: Spectra::from_parts(magnitude, harmonic, percussive, p.sample_rate),
        short_term,
        duration_ms,
        sections: spans,
    }
}

/// The arrangement the ordering test uses: an unambiguous intro, build, drop,
/// breakdown, second drop and outro.
pub fn unambiguous_arrangement() -> Vec<Section> {
    vec![
        Section::new(8, 0.10, 0.00, 0.04, 0.60),
        Section::new(4, 0.55, 0.25, 0.35, 0.70).ramped(),
        Section::new(8, 1.00, 0.70, 0.50, 0.50),
        Section::new(8, 0.04, 0.00, 0.03, 1.00),
        Section::new(8, 1.00, 0.70, 0.50, 0.50),
        Section::new(8, 0.18, 0.05, 0.08, 0.50),
    ]
}

/// Index of each section in [`unambiguous_arrangement`].
pub const INTRO: usize = 0;
pub const BUILD: usize = 1;
pub const DROP: usize = 2;
pub const BREAKDOWN: usize = 3;
pub const DROP_2: usize = 4;
pub const OUTRO: usize = 5;

//! Synthetic drum patterns for the rhythm tests.
//!
//! Test-only, and deliberately synthetic. The point of a click track is that
//! the right answer is arithmetic rather than opinion: a test that asserts
//! 128.0 BPM on a real file is asserting one annotator's reading of that file,
//! and when it breaks nobody can tell whether the detector or the fixture is
//! wrong. Real material belongs in the eval harness, against annotations.

pub const SAMPLE_RATE: u32 = 44_100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Groove {
    /// Kick on every beat, hat on every off-beat eighth. The case everything in
    /// S6..S8 is tuned for.
    FourToTheFloor,
    /// Kick on every beat and nothing else.
    KickOnly,
    /// Hats on every beat and nothing else. The band-split control: whatever
    /// the kick curve does on this fixture is leakage.
    HatsOnly,
    /// Kick on every beat, plus a louder low accent every second beat. The
    /// octave trap: autocorrelation prefers the two-beat period, and only the
    /// kick-band evidence of step 3 can see that every beat carries a kick.
    AccentEveryTwo,
    /// Kick on beats 1 and 3, snare on 3, hats on every eighth. Half-time: the
    /// kick rate is half the hat rate and the question is genuinely ambiguous,
    /// so the engine owes a low confidence rather than a confident answer.
    HalfTime,
    /// Kick on 1 and 3, hats on triplet-swung eighths. Named in ENGINE.md §7.2
    /// as a case that breaks the autocorrelation.
    Swung,
}

pub struct Fixture {
    pub samples: Vec<f32>,
    /// Click times in ms, for comparing a recovered grid against the truth.
    pub beats_ms: Vec<f64>,
}

/// `seconds` of `groove` at `bpm`.
pub fn click_track(bpm: f64, seconds: f64, groove: Groove) -> Fixture {
    let beat = 60.0 / bpm;
    let n_beats = (seconds / beat).floor() as usize;
    let mut events = Vec::new();
    let mut beats_ms = Vec::new();
    for b in 0..n_beats {
        let t = b as f64 * beat;
        beats_ms.push(t * 1000.0);
        push_beat(&mut events, groove, b, t, beat);
    }
    Fixture { samples: render(&events, seconds), beats_ms }
}

fn push_beat(events: &mut Vec<(f64, Hit)>, groove: Groove, b: usize, t: f64, beat: f64) {
    match groove {
        Groove::KickOnly => events.push((t, Hit::Kick(1.0))),
        Groove::HatsOnly => events.push((t, Hit::Hat(1.0))),
        Groove::FourToTheFloor => {
            events.push((t, Hit::Kick(1.0)));
            events.push((t + beat * 0.5, Hit::Hat(0.6)));
        }
        Groove::AccentEveryTwo => {
            let gain = if b % 2 == 0 { 1.0 } else { 0.55 };
            events.push((t, Hit::Kick(gain)));
        }
        Groove::HalfTime => {
            if b % 2 == 0 {
                events.push((t, Hit::Kick(1.0)));
            } else {
                events.push((t, Hit::Snare(0.8)));
            }
            events.push((t, Hit::Hat(0.5)));
            events.push((t + beat * 0.5, Hit::Hat(0.5)));
        }
        Groove::Swung => {
            if b % 2 == 0 {
                events.push((t, Hit::Kick(1.0)));
            } else {
                events.push((t, Hit::Snare(0.8)));
            }
            events.push((t, Hit::Hat(0.5)));
            // Triplet swing: the off-beat sits two thirds of the way through.
            events.push((t + beat * 2.0 / 3.0, Hit::Hat(0.5)));
        }
    }
}

/// `seconds_each` at `bpm_a` then `seconds_each` at `bpm_b`, both
/// four-to-the-floor, with the second section's grid starting on the boundary.
///
/// The gate on two-decimal BPM exists for exactly this file: a single
/// least-squares fit over the whole grid has a residual far above 15 ms, so the
/// track owes one decimal and a variable-tempo flag.
pub fn tempo_change(bpm_a: f64, bpm_b: f64, seconds_each: f64) -> Fixture {
    let a = click_track(bpm_a, seconds_each, Groove::FourToTheFloor);
    let b = click_track(bpm_b, seconds_each, Groove::FourToTheFloor);
    let offset_ms = seconds_each * 1000.0;
    let mut samples = a.samples;
    samples.extend_from_slice(&b.samples);
    let mut beats_ms = a.beats_ms;
    beats_ms.extend(b.beats_ms.iter().map(|t| t + offset_ms));
    Fixture { samples, beats_ms }
}

/// A quiet pad for `quiet_seconds`, then a four-to-the-floor section.
///
/// The tempogram runs on the densest 60 s window by novelty RMS so that a long
/// ambient intro cannot poison it; this is the fixture that proves the window
/// moves.
pub fn ambient_intro(bpm: f64, quiet_seconds: f64, loud_seconds: f64) -> Fixture {
    let quiet = pad(quiet_seconds);
    let loud = click_track(bpm, loud_seconds, Groove::FourToTheFloor);
    let offset_ms = quiet_seconds * 1000.0;
    let mut samples = quiet;
    samples.extend_from_slice(&loud.samples);
    let beats_ms = loud.beats_ms.iter().map(|t| t + offset_ms).collect();
    Fixture { samples, beats_ms }
}

#[derive(Clone, Copy)]
enum Hit {
    /// 60 Hz, which lands squarely in the 30-250 Hz kick band.
    Kick(f32),
    /// 320 Hz plus noise, in the 250-2500 Hz band.
    Snare(f32),
    /// Differentiated noise, tilted above 2500 Hz.
    Hat(f32),
}

fn render(events: &[(f64, Hit)], seconds: f64) -> Vec<f32> {
    let n = (seconds * SAMPLE_RATE as f64) as usize;
    let mut out = vec![0.0f32; n];
    let mut rng = Lcg(0x2545F491_4F6CDD1D);
    for (t, hit) in events {
        let start = (*t * SAMPLE_RATE as f64) as usize;
        if start >= n {
            continue;
        }
        match hit {
            Hit::Kick(g) => tone(&mut out[start..], 60.0, 0.09, *g),
            Hit::Snare(g) => {
                tone(&mut out[start..], 320.0, 0.05, *g * 0.6);
                noise(&mut out[start..], 0.04, *g * 0.4, &mut rng, false);
            }
            Hit::Hat(g) => noise(&mut out[start..], 0.015, *g, &mut rng, true),
        }
    }
    out
}

fn pad(seconds: f64) -> Vec<f32> {
    let n = (seconds * SAMPLE_RATE as f64) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            0.02 * (2.0 * std::f32::consts::PI * 110.0 * t).sin()
        })
        .collect()
}

fn tone(out: &mut [f32], hz: f32, decay_s: f32, gain: f32) {
    let len = ((decay_s * SAMPLE_RATE as f32) as usize).min(out.len());
    for i in 0..len {
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = (-t / (decay_s * 0.3)).exp();
        out[i] += gain * env * (2.0 * std::f32::consts::PI * hz * t).sin();
    }
}

/// `differentiate` tilts white noise up by 6 dB/octave, which is what puts a
/// hat's energy above 2500 Hz where the hat curve can see it.
fn noise(out: &mut [f32], decay_s: f32, gain: f32, rng: &mut Lcg, differentiate: bool) {
    let len = ((decay_s * SAMPLE_RATE as f32) as usize).min(out.len());
    let mut prev = 0.0f32;
    for i in 0..len {
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = (-t / (decay_s * 0.3)).exp();
        let white = rng.next_f32();
        let v = if differentiate { white - prev } else { white };
        prev = white;
        out[i] += gain * env * v * 0.7;
    }
}

/// A fixed-seed generator so a failing test fails the same way twice.
struct Lcg(u64);

impl Lcg {
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / 8_388_608.0) - 1.0
    }
}

//! Unpitched fixtures: percussion, an atonal cluster and silence.
//!
//! These are the cases the key stage has to refuse rather than answer, so they
//! live apart from the tonal generators and cannot be weakened by a change made
//! to make a progression easier to detect.

use super::test_signals::{midi_hz, Osc};

/// All twelve pitch classes sounding at once, sustained.
///
/// Atonal by construction: every key fits equally badly, so the only honest
/// answer is no answer. Percussion tests the "no pitch at all" path; this tests
/// the "pitch everywhere" one, which is the harder of the two to get right
/// because the chroma is full rather than empty.
pub fn chromatic_cluster(sample_rate: u32, seconds: f32) -> Vec<f32> {
    let total = (sample_rate as f32 * seconds) as usize;
    let mut out = vec![0.0f32; total];
    for note in 48..60 {
        for h in 1..=4 {
            let f = midi_hz(note, 0.0) * h as f32;
            if f >= sample_rate as f32 * 0.45 {
                break;
            }
            let mut osc = Osc::new(f, sample_rate);
            let amp = 0.05 / h as f32;
            for s in out.iter_mut() {
                *s += amp * osc.next();
            }
        }
    }
    out
}

/// Deterministic white noise. Seeded, because a test that fails one run in
/// fifty is worse than no test.
struct Lcg(u64);

impl Lcg {
    fn next_f32(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
    }
}

/// Pure percussion: a kick under the chroma high-pass, a noise snare, noise
/// hats. No pitched content, so the only honest answer is that there is none.
pub fn percussion(sample_rate: u32, seconds: f32) -> Vec<f32> {
    let total = (sample_rate as f32 * seconds) as usize;
    let mut out = vec![0.0f32; total];
    let mut rng = Lcg(0x5EED_1234_ABCD_0001);
    let beat = (sample_rate as f32 * 0.5) as usize;
    let sixteenth = beat / 4;
    let mut kick = Osc::new(52.0, sample_rate);
    for i in 0..total {
        let in_beat = i % beat;
        let in_sixteenth = i % sixteenth.max(1);
        let mut v = 0.0f32;
        // Kick on every beat: a fast-decaying low sine, which the 200 Hz
        // high-pass removes, plus the broadband click that it does not.
        let k_env = (-(in_beat as f32) / (sample_rate as f32 * 0.04)).exp();
        v += 0.6 * k_env * kick.next();
        v += 0.25 * k_env * k_env * rng.next_f32();
        // Hats on every sixteenth.
        let h_env = (-(in_sixteenth as f32) / (sample_rate as f32 * 0.01)).exp();
        v += 0.18 * h_env * rng.next_f32();
        // Snare on the backbeat.
        if (i / beat) % 2 == 1 {
            let s_env = (-(in_beat as f32) / (sample_rate as f32 * 0.08)).exp();
            v += 0.3 * s_env * rng.next_f32();
        }
        out[i] = v;
    }
    out
}

pub fn silence(sample_rate: u32, seconds: f32) -> Vec<f32> {
    vec![0.0; (sample_rate as f32 * seconds) as usize]
}

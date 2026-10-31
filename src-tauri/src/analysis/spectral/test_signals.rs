//! Signals whose answer is known, for the spectral and key tests.
//!
//! Test-only. Chord progressions in every key, a track detuned by a stated
//! number of cents, atonal material, percussion and silence all come from here,
//! so one generator bug cannot flatter one stage's tests while hiding from
//! another's.

use std::f32::consts::PI;

/// Test rate. Half the production 44.1 kHz because the tests run in a debug
/// build; a smaller `n_fft`-to-rate ratio also gives the low end *better*
/// resolution, so nothing is made artificially easy. One case runs at 44,100.
pub const TEST_RATE: u32 = 22_050;

/// A numerically stable sinusoid: a unit phasor multiplied each sample and
/// renormalised periodically.
///
/// Not `sin()` per sample: 36 cases of 8 seconds of seven voices at six
/// partials is a hundred million transcendental calls in a debug build. Not the
/// two-term recurrence either, which drifts in amplitude over a third of a
/// million samples.
pub struct Osc {
    re: f32,
    im: f32,
    step_re: f32,
    step_im: f32,
    n: u32,
}

impl Osc {
    pub fn new(freq_hz: f32, sample_rate: u32) -> Self {
        let w = 2.0 * PI * freq_hz / sample_rate as f32;
        Self { re: 1.0, im: 0.0, step_re: w.cos(), step_im: w.sin(), n: 0 }
    }

    pub fn next(&mut self) -> f32 {
        let re = self.re * self.step_re - self.im * self.step_im;
        let im = self.re * self.step_im + self.im * self.step_re;
        self.re = re;
        self.im = im;
        self.n += 1;
        if self.n == 4096 {
            let mag = (self.re * self.re + self.im * self.im).sqrt();
            if mag > 0.0 {
                self.re /= mag;
                self.im /= mag;
            }
            self.n = 0;
        }
        self.im
    }
}

/// MIDI note to frequency, detuned in cents the way a turntable would do it:
/// everything at once.
pub fn midi_hz(note: i32, detune_cents: f32) -> f32 {
    440.0 * 2f32.powf((note as f32 - 69.0) / 12.0 + detune_cents / 1200.0)
}

/// A chord: semitone offsets from its root.
pub const MAJOR_TRIAD: [i32; 3] = [0, 4, 7];
pub const MINOR_TRIAD: [i32; 3] = [0, 3, 7];
pub const DOMINANT_SEVENTH: [i32; 4] = [0, 4, 7, 10];

/// One chord in a progression: root offset from the key's tonic, the chord, and
/// its length in beats.
pub struct Chord {
    pub root: i32,
    pub intervals: &'static [i32],
    pub beats: u32,
}

const fn chord(root: i32, intervals: &'static [i32], beats: u32) -> Chord {
    Chord { root, intervals, beats }
}

/// `I - V - vi - IV - I`. The tonic carries twice the length of anything else,
/// which is what lets a profile match tell a key from its relative at all.
pub fn major_pop() -> Vec<Chord> {
    vec![
        chord(0, &MAJOR_TRIAD, 4),
        chord(7, &MAJOR_TRIAD, 2),
        chord(9, &MINOR_TRIAD, 2),
        chord(5, &MAJOR_TRIAD, 2),
        chord(0, &MAJOR_TRIAD, 4),
    ]
}

/// `I - IV - V7 - I`. The dominant seventh puts the leading tone in.
pub fn major_cadence() -> Vec<Chord> {
    vec![
        chord(0, &MAJOR_TRIAD, 4),
        chord(5, &MAJOR_TRIAD, 2),
        chord(7, &DOMINANT_SEVENTH, 2),
        chord(0, &MAJOR_TRIAD, 4),
    ]
}

/// `i - iv - i - bVII - bVI`. Minor with the subdominant minor, so the pitch
/// content is not simply the relative major's.
pub fn minor_iv() -> Vec<Chord> {
    vec![
        chord(0, &MINOR_TRIAD, 4),
        chord(5, &MINOR_TRIAD, 2),
        chord(0, &MINOR_TRIAD, 4),
        chord(10, &MAJOR_TRIAD, 2),
        chord(8, &MAJOR_TRIAD, 2),
    ]
}

/// `bVI - bIII - bVII - i`, every chord the same length.
///
/// **Genuinely ambiguous between a key and its relative**, and the lack of a
/// doubled tonic is the whole point: the pitch content is the relative major's
/// pitch content, and with nothing weighted as the tonic no chroma can separate
/// `C minor` from `Eb major`. The two differ by one Camelot letter, which is
/// exactly the error class the ambiguity flag exists for.
pub fn minor_relative_ambiguous() -> Vec<Chord> {
    vec![
        chord(8, &MAJOR_TRIAD, 2),
        chord(3, &MAJOR_TRIAD, 2),
        chord(10, &MAJOR_TRIAD, 2),
        chord(0, &MINOR_TRIAD, 2),
    ]
}

/// Folds a note into the octave starting at `base`, so every key occupies the
/// same register.
///
/// Without it the fixture transposes its whole arrangement with the key, and a
/// progression seven semitones up loses its top partials to the chroma stage's
/// 3500 Hz ceiling - which read as a detection failure in G while C was fine.
/// Real music does not transpose its register with its key.
fn voice(note: i32, base: i32) -> i32 {
    base + (note - base).rem_euclid(12)
}

/// Renders a progression in `tonic` (0 = C), looped to fill `seconds`.
///
/// Chord tones in two octaves from C4 plus the root in the bass, because the
/// bass is what makes a tonic read as the tonic in a real chroma as well as in
/// this one.
pub fn progression(
    chords: &[Chord],
    tonic: i32,
    sample_rate: u32,
    seconds: f32,
    detune_cents: f32,
) -> Vec<f32> {
    let beat_samples = (sample_rate as f32 * 0.5) as usize; // 120 BPM
    let total = (sample_rate as f32 * seconds) as usize;
    let mut out = vec![0.0f32; total];
    let mut cursor = 0usize;
    'fill: loop {
        for c in chords {
            if cursor >= total {
                break 'fill;
            }
            let end = (cursor + beat_samples * c.beats as usize).min(total);
            // Every chord tone in two octaves, the root once in the bass.
            // Voicing only the root across three octaves - which an earlier
            // draft did - made the tonic ten times the weight of its own third
            // in some keys, and the chroma gate then removed the third
            // altogether. No real arrangement is that top-heavy on one note.
            let mut notes: Vec<i32> = Vec::new();
            for i in c.intervals {
                notes.push(voice(60 + tonic + c.root + i, 60));
                notes.push(voice(60 + tonic + c.root + i, 72));
            }
            notes.push(voice(36 + tonic + c.root, 36));
            render(&mut out[cursor..end], &notes, sample_rate, detune_cents);
            cursor = end;
        }
        if cursor >= total {
            break;
        }
    }
    out
}

/// Adds every note, six partials at `1/h`, over one chord's span.
fn render(span: &mut [f32], notes: &[i32], sample_rate: u32, detune_cents: f32) {
    const PARTIALS: i32 = 6;
    let len = span.len();
    for note in notes {
        for h in 1..=PARTIALS {
            let f = midi_hz(*note, detune_cents) * h as f32;
            if f >= sample_rate as f32 * 0.45 {
                break;
            }
            let amp = 0.08 / h as f32;
            let mut osc = Osc::new(f, sample_rate);
            for (i, s) in span.iter_mut().enumerate() {
                // A short fade in and out, so a chord change is not a click
                // whose broadband spectrum pollutes the chroma.
                *s += amp * fade(i, len) * osc.next();
            }
        }
    }
}

fn fade(i: usize, len: usize) -> f32 {
    let edge = (len / 20).max(1);
    if i < edge {
        i as f32 / edge as f32
    } else if i + edge >= len {
        (len - i) as f32 / edge as f32
    } else {
        1.0
    }
}

/// Harmonics of one pitch, for the tuning tests: the answer is the detune and
/// nothing else.
pub fn tone(note: i32, sample_rate: u32, seconds: f32, detune_cents: f32) -> Vec<f32> {
    let total = (sample_rate as f32 * seconds) as usize;
    let mut out = vec![0.0f32; total];
    for h in 1..=6 {
        let f = midi_hz(note, detune_cents) * h as f32;
        if f >= sample_rate as f32 * 0.45 {
            break;
        }
        let mut osc = Osc::new(f, sample_rate);
        let amp = 0.3 / h as f32;
        for s in out.iter_mut() {
            *s += amp * osc.next();
        }
    }
    out
}

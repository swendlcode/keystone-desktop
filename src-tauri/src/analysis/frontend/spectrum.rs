//! Per-frame spectral operations, plus the one time-domain filter that has to
//! run before the transform.

/// Bands per octave the whitening envelope is sampled at.
///
/// Three is coarse enough that the envelope follows the mastering curve rather
/// than the partials — at one band per semitone it would track the harmonics
/// themselves and whitening would flatten the music away along with the
/// spectral tilt.
const ENVELOPE_BANDS_PER_OCTAVE: f32 = 3.0;

/// Lowest envelope centre. Below this there is nothing to normalise: the
/// chroma path high-passes at 200 Hz and the structure path's lowest band
/// starts at 30 Hz.
const ENVELOPE_F_MIN_HZ: f32 = 25.0;

/// Envelope floor, as a fraction of the frame's loudest envelope point.
///
/// Without a floor an all-but-silent top octave divides something near zero by
/// something near zero and comes back as loud as the fundamental, which is how
/// whitening turns a quiet passage into broadband noise and a key template
/// match into a coin toss.
const ENVELOPE_FLOOR_RATIO: f32 = 1e-3;

/// 200 Hz high-pass before the chroma FFT, killing percussive low-frequency
/// noise that otherwise smears into the bass bins.
///
/// A second-order Butterworth biquad, not one pole: a kick fundamental sits
/// around 50 Hz, two octaves below the corner, where one pole leaves it only
/// 12 dB down — still the loudest thing in a whitened spectrum. Two poles put
/// it 24 dB down, which is the point of the filter. Forward only, because
/// nothing downstream of this reads phase, only magnitude.
pub fn high_pass(samples: &mut [f32], sample_rate: u32, cutoff_hz: f32) {
    let nyquist = sample_rate as f32 * 0.5;
    if samples.is_empty() || cutoff_hz <= 0.0 || cutoff_hz >= nyquist {
        return;
    }
    let w0 = 2.0 * std::f64::consts::PI * cutoff_hz as f64 / sample_rate as f64;
    let (cos_w0, sin_w0) = (w0.cos(), w0.sin());
    // Q = 1/sqrt(2): maximally flat in the passband, no resonant lift at the
    // corner that would exaggerate whatever sits right at 200 Hz.
    let alpha = sin_w0 / std::f64::consts::SQRT_2;
    let a0 = 1.0 + alpha;
    let (b0, b1, b2) = (
        (1.0 + cos_w0) / 2.0 / a0,
        -(1.0 + cos_w0) / a0,
        (1.0 + cos_w0) / 2.0 / a0,
    );
    let (a1, a2) = (-2.0 * cos_w0 / a0, (1.0 - alpha) / a0);

    let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for s in samples.iter_mut() {
        let x0 = *s as f64;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y0;
        *s = y0 as f32;
    }
}

/// Spectral whitening (Röbel & Rodet 2005) on a magnitude frame.
///
/// The envelope is the local peak magnitude in each third-octave band,
/// interpolated between band centres in log-frequency and log-magnitude, and
/// the frame is divided by it. Peaks rather than means, because the envelope is
/// meant to trace the top of the spectrum: a mean envelope is pulled down by
/// the gaps between partials and whitening against it leaves the partials
/// standing proud of it, which is the thing it was supposed to remove.
///
/// What this buys: a template match then turns on which pitch classes are
/// present rather than on how the track was mastered, so a bass-heavy master
/// and a bright one of the same record land on the same profile.
pub fn spectral_whitening(frame: &mut [f32], sample_rate: u32, n_fft: usize) {
    if frame.len() < 3 || n_fft == 0 {
        return;
    }
    let bin_hz = sample_rate as f32 / n_fft as f32;
    if bin_hz <= 0.0 {
        return;
    }
    let env = envelope(frame, bin_hz);
    let peak = env.iter().copied().fold(0.0f32, f32::max);
    if peak <= 0.0 {
        return;
    }
    let floor = peak * ENVELOPE_FLOOR_RATIO;
    for (m, e) in frame.iter_mut().zip(env.iter()) {
        *m /= e.max(floor);
    }
}

/// Third-octave peak envelope, one value per bin.
fn envelope(frame: &[f32], bin_hz: f32) -> Vec<f32> {
    let nyquist = (frame.len() - 1) as f32 * bin_hz;
    let half_band = (0.5f32 / ENVELOPE_BANDS_PER_OCTAVE).exp2();
    let step = (1.0f32 / ENVELOPE_BANDS_PER_OCTAVE).exp2();

    // (centre bin, peak magnitude) per band, in ascending order.
    let mut knots: Vec<(f32, f32)> = Vec::new();
    let mut centre = ENVELOPE_F_MIN_HZ;
    while centre <= nyquist.max(ENVELOPE_F_MIN_HZ) {
        let lo = ((centre / half_band) / bin_hz).floor().max(0.0) as usize;
        let hi = (((centre * half_band) / bin_hz).ceil() as usize).min(frame.len() - 1);
        let peak = frame[lo..=hi.max(lo)]
            .iter()
            .copied()
            .fold(0.0f32, f32::max);
        knots.push((centre / bin_hz, peak));
        centre *= step;
    }
    interpolate(&knots, frame.len())
}

/// Linear interpolation between knots in log magnitude, flat outside them.
///
/// Log magnitude, because the envelope spans tens of dB across the spectrum and
/// a straight line through linear magnitudes between two knots an octave apart
/// sits far above the real envelope in the middle of that octave.
fn interpolate(knots: &[(f32, f32)], n_bins: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; n_bins];
    if knots.is_empty() {
        return out;
    }
    const EPS: f32 = 1e-12;
    let log_at = |i: usize| knots[i].1.max(EPS).ln();
    let mut k = 0usize;
    for (b, slot) in out.iter_mut().enumerate() {
        let x = b as f32;
        while k + 1 < knots.len() && knots[k + 1].0 < x {
            k += 1;
        }
        *slot = if x <= knots[0].0 {
            knots[0].1.max(EPS)
        } else if k + 1 >= knots.len() {
            knots[k].1.max(EPS)
        } else {
            let (x0, x1) = (knots[k].0, knots[k + 1].0);
            let t = if x1 > x0 { (x - x0) / (x1 - x0) } else { 0.0 };
            (log_at(k) + t * (log_at(k + 1) - log_at(k))).exp()
        };
    }
    out
}

/// Magnitude-weighted mean bin, in log-Hz.
///
/// The mean of `ln f`, which is the log of the geometric mean frequency — not
/// the log of the arithmetic centroid. They are different numbers and the
/// difference is not small on a spectrum with any high-frequency tail at all:
/// a few loud hat bins drag an arithmetic centroid upwards far more than they
/// move a geometric one, which is why the energy model's `centroid` feature,
/// built to separate a dark track from a bright one, is specified in log-Hz.
/// Bin 0 is DC and has no log frequency, so it is excluded.
pub fn spectral_centroid(frame: &[f32], bin_hz: f32) -> f32 {
    if frame.len() < 2 || bin_hz <= 0.0 {
        return 0.0;
    }
    let mut total = 0.0f64;
    let mut weighted = 0.0f64;
    for (b, m) in frame.iter().enumerate().skip(1) {
        let m = *m as f64;
        if m <= 0.0 {
            continue;
        }
        total += m;
        weighted += m * (b as f64 * bin_hz as f64).ln();
    }
    if total <= 0.0 {
        return 0.0;
    }
    (weighted / total) as f32
}

/// Parabolic interpolation around a spectral peak, returning the sub-bin
/// position. The tuning estimate has no sub-semitone resolution without it.
///
/// Fitted through the **log** magnitudes: the main lobe of a Hann-windowed
/// sinusoid is very nearly a parabola in dB, and interpolating there puts the
/// frequency error under a hundredth of a bin, where a linear-magnitude fit is
/// an order of magnitude worse. At 4096 points and 44.1 kHz one bin is 10.8 Hz,
/// which at 440 Hz is 42 cents — wider than the whole ±50 cent range S2 is
/// trying to measure, so without this the stage has nothing to report.
pub fn parabolic_peak(mags: &[f32], bin: usize) -> f32 {
    if bin == 0 || bin + 1 >= mags.len() {
        return bin as f32;
    }
    const EPS: f32 = 1e-20;
    let a = mags[bin - 1].max(EPS).ln();
    let b = mags[bin].max(EPS).ln();
    let c = mags[bin + 1].max(EPS).ln();
    let denom = a - 2.0 * b + c;
    if !denom.is_finite() || denom.abs() < 1e-9 {
        return bin as f32;
    }
    let delta = 0.5 * (a - c) / denom;
    if !delta.is_finite() {
        return bin as f32;
    }
    // A vertex more than half a bin away means a neighbour is the real peak.
    // Clamping keeps the answer inside the bin the caller asked about instead
    // of silently reporting a different peak's position.
    bin as f32 + delta.clamp(-0.5, 0.5)
}

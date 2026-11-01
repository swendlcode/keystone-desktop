//! The transform itself: the window forms, the STFT's scaling and centring, and
//! the harmonic/percussive split built on it.

use ndarray::Array2;

use super::{bin_sine, noise, opts, tone_and_click, SR};
use crate::analysis::frontend::{self, Spectrogram, WindowKind};

#[test]
fn hann_forms_differ_only_at_the_far_edge() {
    let sym = frontend::window(WindowKind::Hann, 8);
    assert!(sym[0].abs() < 1e-6);
    assert!(sym[7].abs() < 1e-6);
    for i in 0..4 {
        assert!((sym[i] - sym[7 - i]).abs() < 1e-6);
    }
    let per = frontend::window(WindowKind::HannPeriodic, 8);
    assert!(per[7] > 0.0);

    // The coherent gains the magnitude scaling depends on: n/2 periodic,
    // (n-1)/2 symmetric.
    let n = 1024;
    let gain = |w: Vec<f32>| w.iter().sum::<f32>();
    assert!((gain(frontend::window(WindowKind::HannPeriodic, n)) - n as f32 / 2.0).abs() < 1e-2);
    assert!(
        (gain(frontend::window(WindowKind::Hann, n)) - (n - 1) as f32 / 2.0).abs() < 1e-2
    );
    assert_eq!(frontend::window(WindowKind::Hann, 1), vec![1.0]);
}

#[test]
fn a_pure_sine_lands_in_one_bin_at_the_predicted_magnitude() {
    const N: usize = 1024;
    const BIN: usize = 64;
    const A: f32 = 0.5;
    let samples = bin_sine(BIN, N, A, N);
    let spec = frontend::stft_magnitude(&samples, SR, &opts(N, N, false)).unwrap();
    assert_eq!(spec.n_frames(), 1);
    assert_eq!(spec.n_bins(), N / 2 + 1);

    // |X[k]| = A * sum(w) / 2 for a sine at bin k: the window's coherent gain
    // halved, because half the amplitude sits at the mirrored negative
    // frequency.
    let gain: f32 = frontend::window(WindowKind::Hann, N).iter().sum();
    let expected = A * gain / 2.0;
    let got = spec.data[(0, BIN)];
    assert!(
        (got - expected).abs() / expected < 0.01,
        "bin {BIN} magnitude {got}, expected {expected}"
    );

    // And nowhere else: outside the main lobe the frame is three orders of
    // magnitude down.
    for b in 0..spec.n_bins() {
        if (b as isize - BIN as isize).abs() > 2 {
            assert!(
                spec.data[(0, b)] < expected * 1e-3,
                "leakage into bin {b}: {}",
                spec.data[(0, b)]
            );
        }
    }
    assert!((spec.bin_hz() - SR as f32 / N as f32).abs() < 1e-6);
}

#[test]
fn silence_produces_zeros_and_an_empty_input_produces_no_frames() {
    let spec = frontend::stft_magnitude(&vec![0.0f32; 4096], SR, &opts(1024, 512, true)).unwrap();
    assert!(spec.n_frames() > 0);
    assert!(spec.data.iter().all(|m| *m == 0.0));

    let empty = frontend::stft_magnitude(&[], SR, &opts(1024, 512, true)).unwrap();
    assert_eq!(empty.n_frames(), 0);
    assert_eq!(empty.n_bins(), 513);
}

#[test]
fn parseval_holds_for_one_frame() {
    const N: usize = 512;
    let samples = noise(N);
    let spec = frontend::stft_magnitude(&samples, SR, &opts(N, N, false)).unwrap();
    let win = frontend::window(WindowKind::Hann, N);

    let time: f64 = samples
        .iter()
        .zip(win.iter())
        .map(|(x, w)| {
            let v = (*x * *w) as f64;
            v * v
        })
        .sum();
    // The real transform stores bins 0..=N/2; the discarded half is the
    // conjugate of bins 1..N/2, hence the doubling.
    let row = spec.data.row(0);
    let mut freq = (row[0] as f64).powi(2) + (row[N / 2] as f64).powi(2);
    freq += 2.0 * row.iter().skip(1).take(N / 2 - 1).map(|m| (*m as f64).powi(2)).sum::<f64>();
    freq /= N as f64;

    assert!(
        (time - freq).abs() / time < 1e-4,
        "time {time} vs frequency {freq}"
    );
}

#[test]
fn sqrt_n_normalisation_only_rescales() {
    const N: usize = 512;
    let samples = noise(N * 3);
    let plain = frontend::stft_magnitude(&samples, SR, &opts(N, N, false)).unwrap();
    let mut o = opts(N, N, false);
    o.normalize_by_sqrt_n = true;
    let scaled = frontend::stft_magnitude(&samples, SR, &o).unwrap();
    let factor = (N as f32).sqrt();
    for (p, s) in plain.data.iter().zip(scaled.data.iter()) {
        assert!((p / factor - s).abs() <= 1e-6 * factor);
    }
}

#[test]
fn centring_puts_frame_zero_on_sample_zero() {
    const N: usize = 64;
    let mut click = vec![0.0f32; N * 4];
    click[0] = 1.0;

    // Centred, the click sits under the window's apex, so every bin carries
    // the window value there. Uncentred, frame 0 starts at sample 0 where a
    // symmetric Hann is zero, so the frame is empty — which is exactly the
    // half-window shift that must not be mixed between stages.
    let centred = frontend::stft_magnitude(&click, SR, &opts(N, N / 2, true)).unwrap();
    let apex = frontend::window(WindowKind::Hann, N)[N / 2];
    for b in 0..centred.n_bins() {
        assert!((centred.data[(0, b)] - apex).abs() < 1e-5);
    }
    let plain = frontend::stft_magnitude(&click, SR, &opts(N, N / 2, false)).unwrap();
    assert!(plain.data.row(0).iter().all(|m| *m < 1e-6));
    assert_eq!(centred.n_frames(), 1 + click.len() / (N / 2));
}

#[test]
fn stft_rejects_a_window_the_real_transform_cannot_plan() {
    assert!(frontend::stft_magnitude(&[0.0; 16], SR, &opts(1023, 256, false)).is_err());
    assert!(frontend::stft_magnitude(&[0.0; 16], SR, &opts(1024, 0, false)).is_err());
}

#[test]
fn hpss_splits_a_tone_from_a_click_and_conserves_the_mix() {
    let spec = tone_and_click(40, 24, 20);
    let (h, p) = frontend::hpss(&spec, 17, 17).unwrap();

    // The masks sum to one, so the two components add back to the magnitude
    // cell by cell. S10's percussive ratio is only a ratio if they do.
    for ((m, hv), pv) in spec.data.iter().zip(h.data.iter()).zip(p.data.iter()) {
        assert!((m - (hv + pv)).abs() < 1e-5);
    }

    let tone_bin = spec.n_bins() / 2;
    // The tone's own bin, away from the click, is harmonic.
    assert!(h.data[(5, tone_bin)] > 0.9);
    assert!(p.data[(5, tone_bin)] < 0.1);
    // The click's row, away from the tone, is percussive.
    assert!(p.data[(20, 3)] > 0.9);
    assert!(h.data[(20, 3)] < 0.1);

    let empty = Spectrogram {
        data: Array2::<f32>::zeros((0, 24)),
        n_fft: 4096,
        hop: 512,
        sample_rate: SR,
    };
    assert_eq!(frontend::hpss(&empty, 17, 17).unwrap().0.n_frames(), 0);
}

//! The reductions over a spectrum that already exists: filterbanks, band
//! energy, flux, local normalisation, and the per-frame spectral operations.

use ndarray::Array2;

use super::{bin_sine, opts, SR};
use crate::analysis::frontend::{self, FilterBand, Spectrogram, TriangularFilterbank};

#[test]
fn log_bands_average_and_mel_bands_peak_at_one() {
    const N: usize = 2048;
    let n_bins = N / 2 + 1;
    let bank = frontend::log_filterbank(n_bins, SR, N, 64, 30.0, 22_050.0);
    assert_eq!(bank.n_bands(), 64);

    // A flat spectrum through unit-sum bands reads 1.0 in every band, which is
    // the property the band-limited flux curves are compared under.
    let flat = vec![1.0f32; n_bins];
    let mut out = vec![0.0f32; bank.n_bands()];
    bank.apply(&flat, &mut out);
    for (i, v) in out.iter().enumerate() {
        assert!((v - 1.0).abs() < 1e-4, "band {i} = {v}");
    }
    for band in &bank.bands {
        assert!(!band.weights.is_empty());
        assert!(band.start_bin + band.weights.len() <= n_bins);
    }

    let mel = frontend::mel_filterbank(n_bins, SR, N, 32, 0.0, 22_050.0);
    assert_eq!(mel.n_bands(), 32);
    for band in &mel.bands {
        let peak = band.weights.iter().copied().fold(0.0f32, f32::max);
        assert!((peak - 1.0).abs() < 1e-5);
    }
}

#[test]
fn a_band_is_a_dot_product_from_its_start_bin() {
    let bank = TriangularFilterbank {
        bands: vec![
            FilterBand {
                start_bin: 1,
                weights: vec![0.5, 0.5],
            },
            FilterBand {
                start_bin: 3,
                weights: vec![1.0, 2.0],
            },
        ],
    };
    let mut out = vec![0.0f32; 2];
    bank.apply(&[9.0, 2.0, 4.0, 1.0, 3.0], &mut out);
    assert!((out[0] - 3.0).abs() < 1e-6);
    assert!((out[1] - 7.0).abs() < 1e-6);

    // A frame shorter than the bank is truncated, not a panic.
    bank.apply(&[9.0, 2.0, 4.0], &mut out);
    assert!((out[0] - 3.0).abs() < 1e-6);
    assert_eq!(out[1], 0.0);
}

#[test]
fn log_compression_is_log1p_and_clamps_negatives() {
    let mut frame = vec![0.0, 0.001, 1.0, -0.5];
    frontend::log_compress(&mut frame, 1000.0);
    assert_eq!(frame[0], 0.0);
    assert!((frame[1] - 2.0f32.ln()).abs() < 1e-6);
    assert!((frame[2] - 1001.0f32.ln()).abs() < 1e-4);
    assert_eq!(frame[3], 0.0);
}

#[test]
fn band_energy_sums_the_bins_it_covers() {
    let spec = Spectrogram {
        // bin_hz = 10, so bins sit at 0, 10, 20 ... Hz.
        data: Array2::from_shape_vec((2, 5), vec![1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 1.0, 1.0, 1.0, 1.0])
            .unwrap(),
        n_fft: 10,
        hop: 5,
        sample_rate: 100,
    };
    // [10, 30) is bins 1 and 2; the upper edge is exclusive.
    assert_eq!(frontend::band_energy(&spec, 10.0, 30.0), vec![5.0, 2.0]);
    // An infinite ceiling runs to Nyquist, which is how "above 6 kHz" is said.
    assert_eq!(
        frontend::band_energy(&spec, 20.0, f32::INFINITY),
        vec![12.0, 3.0]
    );
    assert_eq!(frontend::band_energy(&spec, 30.0, 10.0), vec![0.0, 0.0]);
}

#[test]
fn flux_is_the_rectified_rise_only() {
    let bands =
        Array2::from_shape_vec((3, 2), vec![1.0, 1.0, 2.0, 0.0, 2.0, 3.0]).unwrap();
    // Frame 0 is zero by convention; frame 1 rises 1 in band 0 and falls in
    // band 1, and the fall contributes nothing.
    assert_eq!(frontend::hwr_flux(&bands), vec![0.0, 1.0, 3.0]);
}

#[test]
fn local_normalisation_removes_the_level_and_keeps_the_peaks() {
    // Two plateaus an order of magnitude apart, one spike on each. The level
    // itself must disappear — a loud plateau that survived would be a wall of
    // novelty with no peaks in it — while both spikes stay standing.
    let mut curve = vec![1.0f32; 400];
    for v in curve[200..].iter_mut() {
        *v = 10.0;
    }
    curve[100] = 2.0;
    curve[300] = 20.0;
    frontend::local_normalize(&mut curve, 50);

    assert!(curve.iter().all(|v| *v >= 0.0));
    for i in [50usize, 150, 250, 350] {
        assert!(curve[i] < 1e-3, "plateau at {i} survived as {}", curve[i]);
    }
    assert!(curve[100] > 0.0 && curve[300] > curve[100]);
    // Divided by the global sd, so the curve's own spread is one. The spikes
    // keep their relative sizes on purpose: the local mean is what makes the
    // curve comparable across the track, and rescaling each neighbourhood
    // separately would make a tap in the intro as tall as the drop.
    let mean = curve.iter().map(|v| *v as f64).sum::<f64>() / curve.len() as f64;
    let sd = (curve
        .iter()
        .map(|v| (*v as f64 - mean) * (*v as f64 - mean))
        .sum::<f64>()
        / curve.len() as f64)
        .sqrt();
    assert!((sd - 1.0).abs() < 1e-6, "sd {sd}");

    let mut flat = vec![3.0f32; 100];
    frontend::local_normalize(&mut flat, 50);
    assert!(flat.iter().all(|v| v.abs() < 1e-6));
    let mut empty: Vec<f32> = Vec::new();
    frontend::local_normalize(&mut empty, 50);
    assert!(empty.is_empty());
}

#[test]
fn the_high_pass_keeps_the_mids_and_removes_the_sub() {
    let rms = |s: &[f32]| (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt();
    let settled = SR as usize / 10;

    let mut low = bin_sine(40, SR as usize, 1.0, SR as usize);
    frontend::high_pass(&mut low, SR, 200.0);
    // 40 Hz is 2.3 octaves below the corner: two poles put it ~28 dB down.
    assert!(rms(&low[settled..]) < 0.08);

    let mut mid = bin_sine(2000, SR as usize, 1.0, SR as usize);
    frontend::high_pass(&mut mid, SR, 200.0);
    assert!((rms(&mid[settled..]) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02);

    // Nonsense cutoffs leave the signal alone rather than producing NaNs.
    let mut untouched = vec![0.25f32; 16];
    frontend::high_pass(&mut untouched, SR, 0.0);
    assert!(untouched.iter().all(|v| *v == 0.25));
}

#[test]
fn whitening_flattens_a_tilt_without_flattening_the_partials() {
    const N: usize = 2048;
    let n_bins = N / 2 + 1;
    let bin_hz = SR as f32 / N as f32;
    // A 1/f noise floor with two partials of equal height relative to it, one
    // low and one high. Whitening must bring their absolute heights together.
    let mut frame: Vec<f32> = (0..n_bins)
        .map(|b| 1.0 / (b as f32 * bin_hz).max(20.0))
        .collect();
    let low_bin = (200.0 / bin_hz) as usize;
    let high_bin = (4000.0 / bin_hz) as usize;
    frame[low_bin] *= 8.0;
    frame[high_bin] *= 8.0;
    let before = frame[low_bin] / frame[high_bin];

    frontend::spectral_whitening(&mut frame, SR, N);
    let after = frame[low_bin] / frame[high_bin];
    assert!(before > 15.0, "the untilted case would prove nothing");
    assert!(after > 0.4 && after < 2.5, "ratio after whitening: {after}");

    // A silent frame stays silent instead of becoming noise.
    let mut silent = vec![0.0f32; n_bins];
    frontend::spectral_whitening(&mut silent, SR, N);
    assert!(silent.iter().all(|v| *v == 0.0));
}

#[test]
fn the_centroid_is_the_geometric_mean_in_log_hz() {
    // bin_hz = 100: two equal magnitudes at 200 Hz and 800 Hz, whose
    // geometric mean is 400 Hz.
    let mut frame = vec![0.0f32; 16];
    frame[2] = 1.0;
    frame[8] = 1.0;
    let c = frontend::spectral_centroid(&frame, 100.0);
    assert!((c - 400.0f32.ln()).abs() < 1e-4, "centroid {c}");
    assert_eq!(frontend::spectral_centroid(&vec![0.0f32; 16], 100.0), 0.0);
}

#[test]
fn the_parabolic_vertex_recovers_a_sub_bin_offset() {
    // A sine a third of a bin above bin 64, which no bin-resolution estimate
    // could see at all.
    const N: usize = 1024;
    let offset = 0.3f32;
    let samples: Vec<f32> = (0..N)
        .map(|i| {
            (2.0 * std::f32::consts::PI * (64.0 + offset) * i as f32 / N as f32).sin()
        })
        .collect();
    let spec = frontend::stft_magnitude(&samples, SR, &opts(N, N, false)).unwrap();
    let row: Vec<f32> = spec.data.row(0).to_vec();
    let peak = frontend::parabolic_peak(&row, 64);
    assert!((peak - (64.0 + offset)).abs() < 0.05, "peak at {peak}");

    // A symmetric triple has its vertex at the centre, and an edge bin has no
    // triple at all.
    assert!((frontend::parabolic_peak(&[1.0, 4.0, 1.0], 1) - 1.0).abs() < 1e-6);
    assert_eq!(frontend::parabolic_peak(&[1.0, 4.0, 1.0], 0), 0.0);
    assert_eq!(frontend::parabolic_peak(&[2.0, 2.0, 2.0], 1), 1.0);
}

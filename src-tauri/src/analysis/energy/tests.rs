//! §7.3's first two tests, the ones that are free and automated.
//!
//! Test 1 catches the loudness trap on day one: if any of the seven features is
//! secretly a level, the gain cases fail immediately and the feature is
//! mis-specified rather than miscalibrated. Test 2 is the within-track ordering
//! check, which costs nothing because the segmentation already exists and is a
//! genuinely strong test — a scale that cannot tell a breakdown from a drop
//! inside one track will never be right across a library.
//!
//! What is deliberately *not* here: the real MP3 re-encode and the real
//! 44.1 -> 48 kHz resample over fifty real tracks. Those need an encoder, a
//! resampler and a corpus, and they belong to the dev-only eval harness. Their
//! spectral consequences — every partial landing in a different bin, every frame
//! on a different grid, a gentle wide-Q tilt — are exercised here, which is the
//! part a unit test can honestly own.

use crate::analysis::energy::{self, constants, score};
use crate::analysis::structure::{self, fixture};
use crate::models::EnergyFeatures;

/// §7.3 test 1 allows ±1. The gain cases are asserted exactly equal, because
/// every feature is gain-invariant by construction and a one-step drift would
/// mean one of them is not.
const ALLOWED_DRIFT: i32 = 1;

fn track_energy(f: &fixture::Fixture) -> u8 {
    let st = structure::analyze(&f.spectra, &f.beats, f.duration_ms).expect("structure");
    let core = energy::core_region(&f.short_term, &st.grid.downbeats_ms).expect("core region");
    energy::track_energy(&st.reductions, &st.grid, &f.short_term, core)
        .expect("track energy")
        .energy
}

#[test]
fn energy_is_invariant_to_gain_tilt_and_sample_rate() {
    let arrangement = fixture::unambiguous_arrangement();
    let reference = track_energy(&fixture::render(
        &arrangement,
        128.0,
        fixture::Perturbation::default(),
    ));

    for db in [6.0f32, -6.0] {
        let moved = track_energy(&fixture::render(
            &arrangement,
            128.0,
            fixture::Perturbation::gain_db(db),
        ));
        assert_eq!(
            moved, reference,
            "{db:+} dB moved energy from {reference} to {moved}; a feature is measuring level"
        );
    }

    for case in [
        fixture::Perturbation::tilt(1.0),
        fixture::Perturbation::tilt(-1.0),
        fixture::Perturbation::resampled(48_000),
    ] {
        let moved = track_energy(&fixture::render(&arrangement, 128.0, case));
        let drift = moved as i32 - reference as i32;
        assert!(
            drift.abs() <= ALLOWED_DRIFT,
            "{case:?} moved energy from {reference} to {moved}"
        );
    }
}

#[test]
fn section_energies_order_intro_build_drop_and_breakdown_below_drop() {
    let arrangement = fixture::unambiguous_arrangement();
    let f = fixture::render(&arrangement, 128.0, fixture::Perturbation::default());
    let st = structure::analyze(&f.spectra, &f.beats, f.duration_ms).expect("structure");

    // The fixture's own section spans, not S9's detected ones: this test is
    // about the energy scale, and borrowing the segmentation would let a
    // segmentation bug pass it or fail it for the wrong reason.
    let at = |i: usize| {
        energy::region_energy(&st.reductions, &st.grid, &f.short_term, f.sections[i])
            .expect("region energy")
    };
    let (intro, build, drop) = (at(fixture::INTRO), at(fixture::BUILD), at(fixture::DROP));
    let breakdown = at(fixture::BREAKDOWN);

    assert!(intro <= build, "intro {intro} > build {build}");
    assert!(build <= drop, "build {build} > drop {drop}");
    assert!(breakdown < drop, "breakdown {breakdown} >= drop {drop}");
    // And the scale is actually used. A formula that collapsed everything onto
    // one number would satisfy the ordering above with <= and still be useless;
    // the synthetic drop reads 8 and the breakdown 1 on the frozen constants.
    assert!(drop >= 6, "a drop that reads {drop} means the quantile table is off");
    assert!(breakdown <= 3, "a breakdown that reads {breakdown} means the same");
}

#[test]
fn the_two_drops_read_the_same() {
    let arrangement = fixture::unambiguous_arrangement();
    let f = fixture::render(&arrangement, 128.0, fixture::Perturbation::default());
    let st = structure::analyze(&f.spectra, &f.beats, f.duration_ms).expect("structure");
    let at = |i: usize| {
        energy::region_energy(&st.reductions, &st.grid, &f.short_term, f.sections[i]).unwrap()
    };
    // Identical material has to produce an identical number, or "comparable
    // across sections" is not a claim the per-section row can make.
    assert_eq!(at(fixture::DROP), at(fixture::DROP_2));
}

#[test]
fn raw_score_spans_the_scale_and_the_deciles_land_on_integers() {
    let k = constants::constants();
    assert!((k.weights.iter().sum::<f32>() - 1.0).abs() < 1e-5, "weights must sum to 1");

    let floor = EnergyFeatures::default();
    assert_eq!(score::score(&floor, &k).energy, 1);

    let ceiling = EnergyFeatures {
        onset_rate: 1e3,
        flux_mean: 1e3,
        low_ratio: 1.0,
        high_ratio: 1.0,
        centroid: 1e5,
        perc_ratio: 1.0,
        dynamic_span: 1e3,
    };
    assert_eq!(score::score(&ceiling, &k).energy, 10);

    // Each decile boundary must sit on an integer boundary, which is the whole
    // claim the 1-10 scale makes about a library.
    for i in 1..10 {
        let q = score::quantile_map(k.quantiles[i], &k.quantiles, &k.quantile_targets);
        let boundary = 1.0 + 9.0 * q;
        assert!(
            (boundary - (i as f32 + 0.5)).abs() < 1e-3,
            "decile {i} lands at {boundary}, not {}",
            i as f32 + 0.5
        );
    }
}

#[test]
fn normal_cdf_and_trimmed_mean_behave() {
    assert!((score::normal_cdf(0.0) - 0.5).abs() < 1e-5);
    assert!((score::normal_cdf(1.96) - 0.975).abs() < 1e-3);
    assert!((score::normal_cdf(-1.96) - 0.025).abs() < 1e-3);

    // 10% each side of twenty values trims two from each tail, so the two
    // outliers vanish and the mean of the rest survives.
    let mut v: Vec<f32> = (0..18).map(|_| 5.0).chain([0.0, 100.0]).collect();
    assert!((score::trimmed_mean(&mut v, 0.10) - 5.0).abs() < 1e-5);

    let mut single = vec![7.0f32];
    assert!((score::trimmed_mean(&mut single, 0.10) - 7.0).abs() < 1e-5);
}

#[test]
fn core_region_excludes_a_quiet_intro_and_outro() {
    let downbeats: Vec<f64> = (0..40).map(|i| i as f64 * 2000.0).collect();
    // 100 ms steps: 20 s quiet, 40 s loud, 20 s quiet. The window centre offset
    // is -1.5 s, so the run that comes back is the loud span less that offset.
    let lufs: Vec<f32> = (0..800)
        .map(|i| if (200..600).contains(&i) { -8.0 } else { -26.0 })
        .collect();
    let st = energy::ShortTerm::from_values(lufs, 100.0);
    let (start, end) = energy::core_region(&st, &downbeats).expect("core");

    assert!(start > 15_000.0 && start < 24_000.0, "core starts at {start}");
    assert!(end > 54_000.0 && end < 62_000.0, "core ends at {end}");
}

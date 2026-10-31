//! S9's own checks: the SSM, the checkerboard kernel's two padding modes, and
//! the segmentation over the synthetic arrangement.

use ndarray::{arr2, Array2};

use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::{self, fixture, PadMode, StructureFeature, KERNEL_STRONG_BEATS};
use crate::models::RegionKind;

#[test]
fn distance_matrix_is_zero_on_the_diagonal_and_symmetric() {
    let f = arr2(&[[0.0f32, 1.0], [1.0, 0.0], [0.0, 1.0]]);
    let d = structure::ssm(f.view());
    for i in 0..3 {
        assert_eq!(d[[i, i]], 0.0);
        for j in 0..3 {
            assert!((d[[i, j]] - d[[j, i]]).abs() < 1e-6);
        }
    }
    // Rows 0 and 2 are identical, so they are closer to each other than to 1.
    assert!(d[[0, 2]] < d[[0, 1]]);
}

#[test]
fn a_zero_variance_dimension_does_not_produce_infinities() {
    let mut f = Array2::<f32>::zeros((4, 2));
    for i in 0..4 {
        f[[i, 0]] = 3.0;
        f[[i, 1]] = i as f32;
    }
    let d = structure::ssm(f.view());
    assert!(d.iter().all(|v| v.is_finite()));
}

#[test]
fn zero_padding_gives_the_intro_a_novelty_value_and_valid_does_not() {
    // One block of A then one block of B: a single boundary in the middle, and
    // nothing at the head unless the padding invents it.
    let n = 64;
    let mut f = Array2::<f32>::zeros((n, 1));
    for i in n / 2..n {
        f[[i, 0]] = 1.0;
    }
    let d = structure::ssm(f.view());

    let valid = structure::checkerboard_novelty(d.view(), KERNEL_STRONG_BEATS, PadMode::Valid);
    let zero = structure::checkerboard_novelty(d.view(), KERNEL_STRONG_BEATS, PadMode::Zero);

    let half = KERNEL_STRONG_BEATS / 2;
    assert!(
        valid[..half].iter().all(|v| *v == 0.0),
        "valid padding must leave the head undefined"
    );
    assert!(
        zero[..half].iter().any(|v| *v > 0.0),
        "zero padding is the only reason CUE 1 has anything to find"
    );
    // The real boundary still wins in both modes.
    let peak = zero
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    assert!(peak.abs_diff(n / 2) <= half, "boundary found at {peak}");
}

#[test]
fn the_two_loudness_features_are_the_zero_padded_ones() {
    for f in StructureFeature::ALL {
        let expected = matches!(
            f,
            StructureFeature::HarmonicEnergy | StructureFeature::PercussiveEnergy
        );
        assert_eq!(f.pad_mode() == PadMode::Zero, expected, "{f:?}");
    }
}

#[test]
fn strong_beat_features_are_two_per_bar_and_the_drop_outweighs_the_breakdown() {
    let f = fixture::render(
        &fixture::unambiguous_arrangement(),
        128.0,
        fixture::Perturbation::default(),
    );
    let st = structure::analyze(&f.spectra, &f.beats, f.duration_ms).expect("structure");
    let grid = Grid::from_beats(&f.beats);
    assert_eq!(grid.strong_per_bar, 2);

    let perc = st.features.scalar(StructureFeature::PercussiveEnergy);
    assert_eq!(perc.len(), st.features.n_strong_beats);

    let mean_in = |span: (f64, f64)| {
        let lo = grid.nearest_strong_beat(span.0);
        let hi = grid.nearest_strong_beat(span.1).max(lo + 1).min(perc.len());
        perc[lo..hi].iter().sum::<f32>() / (hi - lo) as f32
    };
    assert!(mean_in(f.sections[fixture::DROP]) > mean_in(f.sections[fixture::BREAKDOWN]));
}

#[test]
fn segmentation_finds_boundaries_and_opens_with_an_intro() {
    let f = fixture::render(
        &fixture::unambiguous_arrangement(),
        128.0,
        fixture::Perturbation::default(),
    );
    let st = structure::analyze(&f.spectra, &f.beats, f.duration_ms).expect("structure");

    assert!(!st.boundaries.is_empty(), "no boundaries at all");
    assert!(
        st.boundaries.iter().all(|b| b.phrase_level >= 4),
        "every boundary carries a phrase level"
    );
    assert!(st.regions.len() >= 2, "regions: {}", st.regions.len());
    assert_eq!(st.regions[0].kind, RegionKind::Intro);
    // Regions tile the track without gaps or overlaps, which the waveform's
    // per-section energy labels depend on.
    for pair in st.regions.windows(2) {
        assert!((pair[0].end_ms - pair[1].start_ms).abs() < 1.0);
    }
}

#[test]
fn snapping_prefers_downbeats_and_falls_back_to_strong_beats() {
    let f = fixture::render(
        &fixture::unambiguous_arrangement(),
        128.0,
        fixture::Perturbation::default(),
    );
    let mut beats = f.beats;
    let grid = Grid::from_beats(&beats);
    let bar = grid.bar_ms;
    assert!((grid.snap_ms(bar * 3.0 + 40.0) - bar * 3.0).abs() < 1.0);

    beats.downbeat_confidence = 0.1;
    let unsure = Grid::from_beats(&beats);
    assert!(!unsure.snap_to_downbeat);
    // Half a bar in, an unsure grid snaps to the strong beat rather than
    // committing to a downbeat a whole bar away.
    let target = bar * 3.0 + bar / 2.0;
    assert!((unsure.snap_ms(target) - target).abs() < 1.0);
}

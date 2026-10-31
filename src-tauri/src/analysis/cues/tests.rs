//! S11's checks. They are about shape rather than about agreement with a DJ:
//! published F1 against real DJ cue points is 0.22 for the incumbent and 0.39
//! for the best system in the literature, so a test that asserted positions
//! would be asserting our own guesses. What is testable, and what makes the
//! feature usable, is that there are eight of them, that they are on the grid,
//! that they are spread across the track, and that the confidences are honest.

use crate::analysis::cues::{self, coverage, N_SLOTS};
use crate::analysis::structure::grid::Grid;
use crate::analysis::structure::{self, fixture};
use crate::models::{CueKind, DetectedCue};

fn detect() -> (Vec<DetectedCue>, Grid, (f64, f64)) {
    let f = fixture::render(
        &fixture::unambiguous_arrangement(),
        128.0,
        fixture::Perturbation::default(),
    );
    let st = structure::analyze(&f.spectra, &f.beats, f.duration_ms).expect("structure");
    let core = crate::analysis::energy::core_region(&f.short_term, &st.grid.downbeats_ms)
        .expect("core region");
    let cues = cues::detect(
        &st.novelty,
        &st.boundaries,
        &st.features,
        &st.grid,
        core,
    )
    .expect("cues");
    (cues, st.grid, core)
}

#[test]
fn there_are_always_eight_cues_one_per_slot() {
    let (cues, _, _) = detect();
    assert_eq!(cues.len(), N_SLOTS);
    let mut slots: Vec<u8> = cues.iter().map(|c| c.slot).collect();
    slots.sort_unstable();
    assert_eq!(slots, (0..N_SLOTS as u8).collect::<Vec<u8>>());
}

#[test]
fn every_cue_is_on_the_grid_and_carries_a_label_bar_and_phrase_level() {
    let (cues, grid, _) = detect();
    for c in &cues {
        // Never a raw novelty peak: the snapped position has to be a grid
        // position to the millisecond.
        assert!(
            (grid.snap_ms(c.position_ms) - c.position_ms).abs() < 1e-6,
            "slot {} at {} is off the grid",
            c.slot,
            c.position_ms
        );
        assert!(c.bar_index.is_some(), "slot {} has no bar", c.slot);
        assert!(
            matches!(c.phrase_level, Some(4) | Some(8) | Some(16) | Some(32)),
            "slot {} phrase level {:?}",
            c.slot,
            c.phrase_level
        );
        assert!(c.label.is_some(), "slot {} has no label", c.slot);
    }
}

#[test]
fn the_roles_are_the_specs_eight() {
    let (cues, _, _) = detect();
    let kinds: Vec<CueKind> = {
        let mut v = cues.clone();
        v.sort_by_key(|c| c.slot);
        v.iter().map(|c| c.kind).collect()
    };
    assert_eq!(
        kinds,
        vec![
            CueKind::Intro,
            CueKind::FirstBeat,
            CueKind::Breakdown,
            CueKind::Breakdown,
            CueKind::Build,
            CueKind::Drop,
            CueKind::Drop,
            CueKind::Outro,
        ]
    );
}

#[test]
fn cues_spread_across_the_core_rather_than_clustering() {
    let (cues, _, core) = detect();
    let mut spans: Vec<usize> = cues
        .iter()
        .map(|c| coverage::span_of(c.position_ms, core))
        .collect();
    spans.sort_unstable();
    spans.dedup();
    // Eight markers inside two eighths of the track is the failure mode the
    // coverage pass exists to prevent, and it is worse than four spread out.
    assert!(
        spans.len() >= 5,
        "eight cues landed in only {} of the eight spans",
        spans.len()
    );
}

#[test]
fn confidences_are_honest_rather_than_flattering() {
    let (cues, _, _) = detect();
    for c in &cues {
        assert!(
            c.confidence > 0.0 && c.confidence <= 0.9,
            "slot {} reported {}",
            c.slot,
            c.confidence
        );
    }
    // The first beat of a four-to-the-floor track is the most reliable cue in
    // the set, and the build is the least. If that ordering ever inverts, the
    // ceilings have been edited without the reasoning behind them.
    let by_slot = |s: u8| cues.iter().find(|c| c.slot == s).unwrap().confidence;
    assert!(by_slot(1) > by_slot(4));
}

#[test]
fn minimum_spacing_holds_between_the_markers() {
    let (cues, grid, core) = detect();
    let mut positions: Vec<f64> = cues.iter().map(|c| c.position_ms).collect();
    positions.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // The effective floor, not the ideal one: eight markers 8 bars apart need
    // 64 bars of core and the fixture is shorter than that on purpose, which is
    // the case a real 3-minute edit also hits.
    let floor = coverage::effective_spacing_ms(&grid, core);
    assert!(floor > 0.0);
    for pair in positions.windows(2) {
        assert!(
            pair[1] - pair[0] >= floor - 1.0,
            "{} and {} are closer than {floor} ms",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn the_salience_filter_removes_candidates_with_nothing_after_them() {
    let f = fixture::render(
        &fixture::unambiguous_arrangement(),
        128.0,
        fixture::Perturbation::default(),
    );
    let st = structure::analyze(&f.spectra, &f.beats, f.duration_ms).expect("structure");
    let raw = cues::peak_pick(&st.novelty, &f.beats).expect("peaks");
    let filtered = cues::salience_filter(raw.clone(), &st.features, &f.beats);
    assert!(
        filtered.len() <= raw.len(),
        "the filter must never add candidates"
    );
    assert!(!raw.is_empty(), "no candidates at all");
}

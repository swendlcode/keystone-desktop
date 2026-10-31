//! ENGINE.md 7.3 tests 1 and 2, the two that are free and automated.
//!
//! Test 1 is the one that catches the loudness trap: if any of the seven
//! features is secretly a level, the gain cases fail immediately and the feature
//! is **mis-specified rather than miscalibrated**. Loudness alone explains
//! arousal at R2 ~ 0.43-0.55 while timbre feature sets reach 0.69-0.75, and
//! every modern master sits near -8 LUFS, so a scale that moved with level would
//! give almost every record in a library the same number *and* change its mind
//! when the user normalised a file.
//!
//! Test 2 is within-track ordering: `intro <= build <= drop` and
//! `breakdown < drop`. The segmentation already exists so it costs nothing, and
//! a scale that cannot tell a breakdown from a drop inside one track will never
//! be right across a library.
//!
//! **What these tests are not.** Every figure here is a synthetic arrangement
//! built in the spectral domain, because the time-domain front end that S9 and
//! S10 share (`analysis::frontend`'s STFT and HPSS) is still a stub and no real
//! file can reach the energy stage yet. The perturbations are therefore what
//! each process *does to a spectrum*, and the re-encode is a surrogate for a
//! codec's in-band effects rather than a round trip through an encoder. Tests 1
//! and 2 are not passed until they have run over 50 real tracks and ~100 real
//! edit pairs; tests 3, 4 and 5 (edit pairs, 300 human pairwise anchors, and the
//! cross-genre consistency check that answers the actual requirement) need
//! listening and are not automatable at all.

#![cfg(feature = "eval")]

use keystone_lib::analysis::energy;
use keystone_lib::analysis::structure::{self, fixture};
use keystone_lib::analysis::structure::fixture::{Perturbation, Section};

/// 7.3 test 1 allows +-1 of drift. The gain cases are held to exact equality,
/// because every feature is gain-invariant by construction and a one-step drift
/// would mean one of them is not.
const ALLOWED_DRIFT: i32 = 1;

const BPM: f64 = 128.0;

/// Whether an arrangement has the unambiguous structure 7.3 test 2 asks for.
const STRICT: bool = true;

/// Six arrangements rather than one. The scale's requirement is that a 7 means
/// the same thing on techno as on house, and the cheapest version of that test
/// is material whose density, bass weight and brightness differ by more than the
/// perturbations do.
fn arrangements() -> Vec<(&'static str, bool, Vec<Section>)> {
    vec![
        ("canonical", STRICT, fixture::unambiguous_arrangement()),
        (
            "techno: dense, dark, bass-heavy",
            STRICT,
            vec![
                Section::new(8, 0.20, 0.00, 0.05, 0.50),
                Section::new(4, 0.70, 0.30, 0.20, 0.55).ramped(),
                Section::new(8, 1.00, 0.55, 0.25, 0.45),
                Section::new(8, 0.06, 0.00, 0.04, 0.90),
                Section::new(8, 1.00, 0.55, 0.25, 0.45),
                Section::new(8, 0.22, 0.04, 0.06, 0.45),
            ],
        ),
        (
            "house: mid-weighted, bright hats",
            STRICT,
            vec![
                Section::new(8, 0.12, 0.02, 0.10, 0.70),
                Section::new(4, 0.50, 0.30, 0.45, 0.75).ramped(),
                Section::new(8, 0.85, 0.60, 0.70, 0.65),
                Section::new(8, 0.05, 0.00, 0.08, 1.00),
                Section::new(8, 0.85, 0.60, 0.70, 0.65),
                Section::new(8, 0.15, 0.05, 0.12, 0.60),
            ],
        ),
        (
            "drum and bass: busy and snare-led",
            STRICT,
            vec![
                Section::new(8, 0.10, 0.05, 0.08, 0.55),
                Section::new(4, 0.45, 0.60, 0.40, 0.60).ramped(),
                Section::new(8, 0.80, 1.00, 0.60, 0.50),
                Section::new(8, 0.04, 0.02, 0.05, 0.95),
                Section::new(8, 0.80, 1.00, 0.60, 0.50),
                Section::new(8, 0.12, 0.08, 0.10, 0.50),
            ],
        ),
        (
            "hip-hop: sparse and heavy",
            STRICT,
            vec![
                Section::new(8, 0.18, 0.04, 0.03, 0.75),
                Section::new(4, 0.55, 0.25, 0.10, 0.80).ramped(),
                Section::new(8, 0.95, 0.45, 0.14, 0.70),
                Section::new(8, 0.08, 0.00, 0.02, 0.95),
                Section::new(8, 0.95, 0.45, 0.14, 0.70),
                Section::new(8, 0.20, 0.06, 0.04, 0.70),
            ],
        ),
        (
            "ambient: almost no percussion",
            // Not unambiguous structure: a 0.16 kick under a pad is not a drop,
            // and 7.3 test 2 asks for the ordering on tracks that have one.
            !STRICT,
            vec![
                Section::new(8, 0.03, 0.00, 0.01, 0.80),
                Section::new(4, 0.08, 0.02, 0.03, 0.90).ramped(),
                Section::new(8, 0.16, 0.06, 0.06, 1.00),
                Section::new(8, 0.02, 0.00, 0.01, 0.85),
                Section::new(8, 0.16, 0.06, 0.06, 1.00),
                Section::new(8, 0.04, 0.00, 0.02, 0.75),
            ],
        ),
    ]
}

/// Track energy over the core region, which is the number the library column
/// shows. Deliberately drop-weighted: the core excludes the quiet intro and
/// outro and the trimmed mean over what is left is dominated by whatever the
/// track spends most of its length doing.
fn track_energy(sections: &[Section], p: Perturbation) -> u8 {
    let f = fixture::render(sections, BPM, p);
    let st = structure::analyze(&f.spectra, &f.beats, f.duration_ms).expect("structure");
    let core = energy::core_region(&f.short_term, &st.grid.downbeats_ms).expect("core region");
    energy::track_energy(&st.reductions, &st.grid, &f.short_term, core)
        .expect("track energy")
        .energy
}

#[test]
fn energy_survives_gain_reencode_resample_and_a_gentle_eq() {
    let mut failures: Vec<String> = Vec::new();
    for (name, _strict, sections) in arrangements() {
        let reference = track_energy(&sections, Perturbation::default());

        // Gain: exact equality. This is the loudness trap and there is no
        // tolerance for it.
        for db in [6.0f32, -6.0, 12.0, -12.0] {
            let moved = track_energy(&sections, Perturbation::gain_db(db));
            println!("{name:38} gain {db:+6.1} dB   {reference} -> {moved}");
            if moved != reference {
                failures.push(format!(
                    "{name}: {db:+} dB moved energy from {reference} to {moved}; a feature \
                     is measuring level"
                ));
            }
        }

        // Everything else: +-1, which is 7.3's own allowance.
        // 7.3 names 44.1 <-> 48 kHz, and that is the pair a DJ library actually
        // contains. A 32 kHz re-render moves the techno arrangement by two
        // steps, but at 32 kHz the fixture's own frame rate and bin width both
        // change as well, so that result cannot be attributed to the energy
        // model rather than to the fixture. It needs the real time-domain front
        // end to measure, and it is recorded as unmeasured rather than asserted
        // either way.
        let cases = [
            ("eq tilt +1 dB/decade", Perturbation::tilt(1.0)),
            ("eq tilt -1 dB/decade", Perturbation::tilt(-1.0)),
            ("resample 48 kHz", Perturbation::resampled(48_000)),
            ("re-encode 320 kbps", Perturbation::reencode(320)),
            ("re-encode 192 kbps", Perturbation::reencode(192)),
            ("re-encode 128 kbps", Perturbation::reencode(128)),
        ];
        for (label, case) in cases {
            let moved = track_energy(&sections, case);
            let drift = moved as i32 - reference as i32;
            println!("{name:38} {label:22} {reference} -> {moved}  ({drift:+})");
            if drift.abs() > ALLOWED_DRIFT {
                failures.push(format!(
                    "{name}: {label} moved energy from {reference} to {moved}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "invariance failures:\n  {}", failures.join("\n  "));
}

#[test]
fn the_scale_is_used_across_arrangements_rather_than_collapsing_onto_one_number() {
    // Invariance is only half the requirement: a formula that answered 5 for
    // everything would pass the test above perfectly. Six arrangements whose
    // density and bass weight differ by far more than any perturbation have to
    // produce more than one answer between them, and the two that are
    // deliberately at the ends have to be on the right sides of the scale.
    let mut seen: Vec<(String, u8)> = Vec::new();
    for (name, _strict, sections) in arrangements() {
        let e = track_energy(&sections, Perturbation::default());
        println!("{name:38} energy {e}");
        seen.push((name.to_string(), e));
    }
    let values: Vec<u8> = seen.iter().map(|(_, e)| *e).collect();
    let distinct = {
        let mut v = values.clone();
        v.sort_unstable();
        v.dedup();
        v.len()
    };
    assert!(distinct >= 3, "six arrangements produced {distinct} distinct energies: {seen:?}");
    let ambient = seen.iter().find(|(n, _)| n.starts_with("ambient")).expect("ambient").1;
    let techno = seen.iter().find(|(n, _)| n.starts_with("techno")).expect("techno").1;
    assert!(
        ambient < techno,
        "ambient read {ambient} and techno {techno}; the scale is not ordering material"
    );
    // And every value is inside the published 1-10 scale.
    for (name, e) in &seen {
        assert!((1..=10).contains(e), "{name} read {e}, outside 1-10");
    }
}

#[test]
fn section_energies_order_intro_build_drop_and_breakdown_below_drop() {
    let mut failures: Vec<String> = Vec::new();
    for (name, strict, sections) in arrangements() {
        let f = fixture::render(&sections, BPM, Perturbation::default());
        let st = structure::analyze(&f.spectra, &f.beats, f.duration_ms).expect("structure");
        // The fixture's own section spans, not S9's detected ones: this test is
        // about the energy scale, and borrowing the segmentation would let a
        // segmentation bug pass it or fail it for the wrong reason.
        let at = |i: usize| {
            energy::region_energy(&st.reductions, &st.grid, &f.short_term, f.sections[i])
                .expect("region energy")
        };
        let (intro, build, drop) = (at(fixture::INTRO), at(fixture::BUILD), at(fixture::DROP));
        let (breakdown, drop2, outro) =
            (at(fixture::BREAKDOWN), at(fixture::DROP_2), at(fixture::OUTRO));
        println!(
            "{name:38} intro {intro}  build {build}  drop {drop}  breakdown {breakdown}  \
             drop2 {drop2}  outro {outro}{}",
            if strict { "" } else { "   (weak ordering only)" }
        );
        if intro > build {
            failures.push(format!("{name}: intro {intro} > build {build}"));
        }
        if build > drop {
            failures.push(format!("{name}: build {build} > drop {drop}"));
        }
        if outro > drop {
            failures.push(format!("{name}: outro {outro} > drop {drop}"));
        }
        // `breakdown < drop` strictly, but only where there is a drop to speak
        // of. On the ambient arrangement every section reads 1: the frozen
        // quantile ladder is calibrated on dance music and compresses
        // near-percussion-free material onto the floor of the scale, so there
        // is no contrast left to order. That is a real limitation of the scale
        // and it is reported rather than asserted away.
        if strict && breakdown >= drop {
            failures.push(format!("{name}: breakdown {breakdown} >= drop {drop}"));
        }
        if breakdown > drop {
            failures.push(format!("{name}: breakdown {breakdown} > drop {drop}"));
        }
        // The two drops are the same percussion over a different chord of the
        // fixture's three-chord cycle, so they are comparable rather than
        // identical: a different bass root moves `low_ratio`. One step apart is
        // the claim the per-section row makes; the canonical arrangement is
        // held to exact equality by the stage's own unit test.
        if (drop as i32 - drop2 as i32).abs() > 1 {
            failures.push(format!("{name}: the two drops read {drop} and {drop2}"));
        }
    }
    assert!(failures.is_empty(), "ordering failures:\n  {}", failures.join("\n  "));
}

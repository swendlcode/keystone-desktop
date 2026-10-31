//! S5 — the Camelot mapping, pinned against the frontend.
//!
//! `src/utils/camelot.ts` and `analysis/camelot.rs` have to agree on all 24
//! keys or the wheel filters to a different set from the one the badges show.
//! The mapping itself lives in `analysis::camelot`, which the whole backend
//! already shares; this is the cross-check, and it is a table rather than a
//! restatement of the formula.

use crate::analysis::camelot::Camelot;
use crate::models::KeyMode;

/// The 24 Camelot codes, written out rather than recomputed.
///
/// A table, not a restatement of the formula: the point is to pin the mapping
/// against `src/utils/camelot.ts`, and a test that reruns the same arithmetic
/// would agree with a transposed wheel just as happily. 0 = C.
const CAMELOT_TABLE: [(u8, KeyMode, &str); 24] = [
    (0, KeyMode::Major, "8B"),
    (1, KeyMode::Major, "3B"),
    (2, KeyMode::Major, "10B"),
    (3, KeyMode::Major, "5B"),
    (4, KeyMode::Major, "12B"),
    (5, KeyMode::Major, "7B"),
    (6, KeyMode::Major, "2B"),
    (7, KeyMode::Major, "9B"),
    (8, KeyMode::Major, "4B"),
    (9, KeyMode::Major, "11B"),
    (10, KeyMode::Major, "6B"),
    (11, KeyMode::Major, "1B"),
    (0, KeyMode::Minor, "5A"),
    (1, KeyMode::Minor, "12A"),
    (2, KeyMode::Minor, "7A"),
    (3, KeyMode::Minor, "2A"),
    (4, KeyMode::Minor, "9A"),
    (5, KeyMode::Minor, "4A"),
    (6, KeyMode::Minor, "11A"),
    (7, KeyMode::Minor, "6A"),
    (8, KeyMode::Minor, "1A"),
    (9, KeyMode::Minor, "8A"),
    (10, KeyMode::Minor, "3A"),
    (11, KeyMode::Minor, "10A"),
];

#[test]
fn camelot_mapping_matches_the_frontend_table_for_all_24_keys() {
    for (root, mode, code) in CAMELOT_TABLE {
        assert_eq!(
            Camelot::from_key(root, mode).label(false),
            code,
            "root {root} {mode:?}"
        );
        let parsed = Camelot::parse(code).unwrap();
        assert_eq!(parsed.to_key(), (root, mode), "{code} did not invert");
    }
}

#[test]
fn camelot_compatibility_is_symmetric_across_the_whole_wheel() {
    // `isCompatible(a, b) === isCompatible(b, a)` for all 24 x 24, which is the
    // property the wheel filter depends on and the one a wrap-around bug breaks.
    let all: Vec<Camelot> =
        CAMELOT_TABLE.iter().map(|(r, m, _)| Camelot::from_key(*r, *m)).collect();
    for a in &all {
        let set = a.compatible();
        assert_eq!(set.len(), 4);
        for b in &all {
            let a_sees_b = set.contains(b);
            let b_sees_a = b.compatible().contains(a);
            assert_eq!(
                a_sees_b,
                b_sees_a,
                "{} and {} disagree about compatibility",
                a.label(false),
                b.label(false)
            );
        }
    }
}

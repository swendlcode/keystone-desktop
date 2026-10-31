//! Tests for [`super`]. Split out so the builder itself stays readable; the
//! `#[path]` attribute keeps it part of the same module.

use super::*;
use crate::models::IndexStatus;

fn filters() -> TrackFilters {
    TrackFilters::default()
}

#[test]
fn upper_bound_sorts_above_every_child_path() {
    let lo = "/Users/dj/Music/";
    let hi = path_upper_bound(lo);
    assert!(hi.as_str() > lo);
    for child in ["/Users/dj/Music/a.mp3", "/Users/dj/Music/zzz/deep.aiff"] {
        assert!(
            child > lo && child < hi.as_str(),
            "{child} fell outside the range"
        );
    }
    // A sibling directory must fall outside, or the scope leaks.
    assert!("/Users/dj/Musicals/x.mp3" > hi.as_str() || "/Users/dj/Musicals/x.mp3" < lo);
}

#[test]
fn the_live_predicate_is_spelled_exactly_as_the_partial_indexes_spell_it() {
    // This is the whole point of the constant: a predicate written differently
    // in the query than in the index makes SQLite re-check every row.
    let (sql, params) = build_where(&filters());
    assert_eq!(sql, LIVE_PREDICATE);
    assert!(params.is_empty());
    assert_eq!(LIVE_PREDICATE, "index_status != 'missing'");
}

#[test]
fn an_explicit_status_filter_replaces_the_live_predicate() {
    // Asking for missing rows and then ANDing "not missing" would return
    // nothing, with no visible cause.
    let f = TrackFilters {
        index_statuses: vec![IndexStatus::Missing],
        ..filters()
    };
    let (sql, params) = build_where(&f);
    assert!(sql.contains("index_status IN (?)"));
    assert!(!sql.contains(LIVE_PREDICATE));
    assert_eq!(params.len(), 1);
}

#[test]
fn a_path_scope_is_a_range_scan_and_never_a_like() {
    let f = TrackFilters {
        path_prefix: Some("/Users/dj/Music/".into()),
        ..filters()
    };
    let (sql, params) = build_where(&f);
    assert!(sql.contains("path >= ?") && sql.contains("path < ?"));
    assert!(
        !sql.to_uppercase().contains("LIKE"),
        "LIKE on a path is case-insensitive by default and forces a full scan"
    );
    assert_eq!(params.len(), 2);
}

#[test]
fn the_wheel_filter_is_the_segment_its_relative_and_one_either_way() {
    let selected = Camelot::parse("8A").unwrap();
    let (sql, params) = camelot_clause(&selected, true);
    assert_eq!(sql, "camelot IN (?,?,?,?)");
    let bound: Vec<String> = params
        .iter()
        .map(|v| match v {
            rusqlite::types::Value::Text(t) => t.clone(),
            other => panic!("unexpected bound value {other:?}"),
        })
        .collect();
    assert_eq!(bound, ["8A", "8B", "7A", "9A"]);

    // Without the compatible set it is just the one segment.
    let (sql, params) = camelot_clause(&selected, false);
    assert_eq!(sql, "camelot IN (?)");
    assert_eq!(params.len(), 1);
}

#[test]
fn the_wheel_filter_wraps_at_twelve_and_at_one() {
    for (selected, expected) in [
        ("12B", ["12B", "12A", "11B", "1B"]),
        ("1A", ["1A", "1B", "12A", "2A"]),
    ] {
        let (_, params) = camelot_clause(&Camelot::parse(selected).unwrap(), true);
        let bound: Vec<String> = params
            .iter()
            .map(|v| match v {
                rusqlite::types::Value::Text(t) => t.clone(),
                other => panic!("unexpected bound value {other:?}"),
            })
            .collect();
        assert_eq!(bound, expected, "{selected} did not wrap");
    }
}

#[test]
fn an_unparseable_wheel_value_is_ignored_rather_than_filtering_to_segment_one() {
    let f = TrackFilters {
        camelot: Some("13A".into()),
        camelot_include_compatible: true,
        ..filters()
    };
    let (sql, _) = build_where(&f);
    assert!(!sql.contains("camelot IN"));
}

#[test]
fn the_text_query_goes_through_fts_and_narrows_on_every_term() {
    assert_eq!(
        fts_match_expression("daft punk").as_deref(),
        Some("\"daft\"* AND \"punk\"*")
    );
    // No fuzzy expansion: one word in, one group out. This is the guard against
    // `crowd` being rewritten into `chord`.
    let one = fts_match_expression("crowd").unwrap();
    assert_eq!(one, "\"crowd\"*");
    assert_eq!(one.matches("OR").count(), 0);
}

#[test]
fn fts_terms_that_look_like_operators_stay_words() {
    let expr = fts_match_expression("not or near").unwrap();
    assert_eq!(expr, "\"not\"* AND \"or\"* AND \"near\"*");
    // A quote in the query must not end the FTS string literal early.
    assert_eq!(fts_match_expression("ab\"cd").as_deref(), Some("\"ab\"* AND \"cd\"*"));
}

#[test]
fn a_query_with_no_indexable_token_falls_back_and_nothing_else_does() {
    assert_eq!(fts_match_expression("!!!"), None);
    let f = TrackFilters {
        query: "!!!".into(),
        ..filters()
    };
    let (sql, params) = build_where(&f);
    assert!(sql.contains("instr("));
    assert_eq!(params.len(), 3);

    let f = TrackFilters {
        query: "aphex".into(),
        ..filters()
    };
    let (sql, _) = build_where(&f);
    assert!(sql.contains("tracks_fts MATCH ?"));
    assert!(!sql.contains("instr("));
}

#[test]
fn every_sort_column_carries_a_stable_tiebreak() {
    for sort in [
        TrackSort::Artist,
        TrackSort::Title,
        TrackSort::Album,
        TrackSort::Genre,
        TrackSort::Tempo,
        TrackSort::KeyResult,
        TrackSort::Energy,
        TrackSort::DateAdded,
        TrackSort::Rating,
        TrackSort::CuePointCount,
        TrackSort::Bitrate,
        TrackSort::Year,
        TrackSort::Filename,
        TrackSort::Path,
    ] {
        for dir in [SortDirection::Asc, SortDirection::Desc] {
            let sql = build_order(sort, dir);
            assert!(sql.starts_with("ORDER BY "), "{sql}");
            assert!(
                sql.ends_with(", id ASC"),
                "{sql:?} can swap rows between pages"
            );
        }
    }
}

#[test]
fn the_key_sort_orders_the_wheel_numerically_not_lexically() {
    let sql = build_order(TrackSort::KeyResult, SortDirection::Asc);
    // Without the CAST, 10A sorts before 1A.
    assert!(sql.contains("CAST(substr(camelot, 1, length(camelot) - 1) AS INTEGER)"));
    assert!(sql.contains("substr(camelot, -1)"));
}

#[test]
fn the_improvable_predicate_covers_all_four_reasons() {
    let f = TrackFilters {
        improvable_only: true,
        ..filters()
    };
    let (sql, _) = build_where(&f);
    for reason in [
        "key_mode_ambiguous = 1",
        "key_confidence <",
        "clipped_peaks > 0",
        "analysis_version = 0",
    ] {
        assert!(sql.contains(reason), "{reason} is not in {sql}");
    }
}

#[test]
fn ranges_filter_the_tempo_the_table_shows() {
    let f = TrackFilters {
        tempo_min: Some(120.0),
        tempo_max: Some(130.0),
        energy_min: Some(5),
        rating_min: Some(3),
        genres: vec!["House".into(), "Techno".into()],
        ..filters()
    };
    let (sql, params) = build_where(&f);
    assert!(sql.contains("tempo_display >= ?") && sql.contains("tempo_display <= ?"));
    assert!(
        !sql.contains("tempo_raw"),
        "filtering raw tempo disagrees with the column the DJ is reading"
    );
    assert!(sql.contains("genre COLLATE NOCASE IN (?,?)"));
    // 2 tempo + 1 energy + 1 rating + 2 genres.
    assert_eq!(params.len(), 6);
}

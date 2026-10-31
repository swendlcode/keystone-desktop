//! Fixtures the read and write tests share.

use crate::db::test_support::Harness;
use crate::models::{SortDirection, TrackFilters, TrackQuery, TrackSort};

pub(super) fn query(filters: TrackFilters) -> TrackQuery {
    TrackQuery {
        filters,
        sort: TrackSort::Artist,
        direction: SortDirection::Asc,
        limit: 100,
        offset: 0,
    }
}

/// A library with one track in each of several Camelot segments, so the wheel
/// query has something to be wrong about.
pub(super) fn wheel_library(h: &Harness) {
    for (path, camelot) in [
        ("/m/eight-a.mp3", "8A"),
        ("/m/eight-b.mp3", "8B"),
        ("/m/seven-a.mp3", "7A"),
        ("/m/nine-a.mp3", "9A"),
        ("/m/two-a.mp3", "2A"),
        ("/m/five-b.mp3", "5B"),
    ] {
        let id = h.add(path);
        h.set_analysis_columns(&id, &format!("camelot = '{camelot}', analysis_version = 1"));
    }
}

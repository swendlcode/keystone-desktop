//! Text <-> enum mapping for the columns migration 001 stores as TEXT, plus the
//! two or three primitives every repository needs.
//!
//! This lives in the db layer rather than on the model types because the
//! strings are a *storage* format: migration 001 spells them out, and changing
//! one is a migration rather than a rename. A `parse` on the model would invite
//! a command or a UI layer to invent its own spelling.

use crate::models::{CollectionKind, CueSource, IndexStatus, ValueSource};

/// How many bound parameters one `IN (...)` list may carry.
///
/// SQLite's compiled limit is 32,766 on a modern build and 999 on an older one.
/// A chunk this size is far under both and keeps the statement cache useful,
/// which matters because the reconciler runs thousands of these.
pub const IN_CHUNK: usize = 500;

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// `?,?,?` for an `IN (...)` list of `n` bound values.
pub fn placeholders(n: usize) -> String {
    let mut s = String::with_capacity(n * 2);
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push('?');
    }
    s
}

pub fn value_source_str(v: ValueSource) -> &'static str {
    match v {
        ValueSource::Detected => "detected",
        ValueSource::User => "user",
        ValueSource::Tag => "tag",
    }
}

pub fn value_source(s: Option<&str>) -> Option<ValueSource> {
    match s? {
        "detected" => Some(ValueSource::Detected),
        "user" => Some(ValueSource::User),
        "tag" => Some(ValueSource::Tag),
        _ => None,
    }
}

pub fn index_status_str(s: IndexStatus) -> &'static str {
    match s {
        IndexStatus::Pending => "pending",
        IndexStatus::Indexed => "indexed",
        IndexStatus::Missing => "missing",
        IndexStatus::Error => "error",
    }
}

/// An unrecognised value means a newer build wrote the row. Reading it as
/// `Pending` gets the file re-probed, which is recoverable; reading it as
/// `Missing` would hide the track from every query instead.
pub fn index_status(s: &str) -> IndexStatus {
    match s {
        "indexed" => IndexStatus::Indexed,
        "missing" => IndexStatus::Missing,
        "error" => IndexStatus::Error,
        _ => IndexStatus::Pending,
    }
}

pub fn cue_source_str(s: CueSource) -> &'static str {
    match s {
        CueSource::Detected => "detected",
        CueSource::User => "user",
    }
}

/// Anything that is not literally `user` is treated as detected, so a drifted
/// value is replaceable by the next analysis rather than frozen for ever.
pub fn cue_source(s: &str) -> CueSource {
    match s {
        "user" => CueSource::User,
        _ => CueSource::Detected,
    }
}

pub fn collection_kind_str(k: CollectionKind) -> &'static str {
    match k {
        CollectionKind::User => "user",
        CollectionKind::Smart => "smart",
    }
}

/// Unknown kinds read as `User`: a smart collection whose rule cannot be
/// understood would otherwise show an empty count with no explanation.
pub fn collection_kind(s: &str) -> CollectionKind {
    match s {
        "smart" => CollectionKind::Smart,
        _ => CollectionKind::User,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_lists_match_their_length() {
        assert_eq!(placeholders(0), "");
        assert_eq!(placeholders(1), "?");
        assert_eq!(placeholders(3), "?,?,?");
    }

    #[test]
    fn every_stored_spelling_round_trips() {
        for v in [ValueSource::Detected, ValueSource::User, ValueSource::Tag] {
            assert_eq!(value_source(Some(value_source_str(v))), Some(v));
        }
        for s in [
            IndexStatus::Pending,
            IndexStatus::Indexed,
            IndexStatus::Missing,
            IndexStatus::Error,
        ] {
            assert_eq!(index_status(index_status_str(s)), s);
        }
        for c in [CueSource::Detected, CueSource::User] {
            assert_eq!(cue_source(cue_source_str(c)), c);
        }
        for k in [CollectionKind::User, CollectionKind::Smart] {
            assert_eq!(collection_kind(collection_kind_str(k)), k);
        }
    }

    #[test]
    fn a_drifted_status_stays_visible_rather_than_vanishing() {
        // Reading an unknown status as Missing would hide the row from every
        // query, which is the one failure mode with no way back for the user.
        assert_eq!(index_status("something-newer"), IndexStatus::Pending);
    }
}

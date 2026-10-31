use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CollectionKind {
    User,
    /// Rule-driven, e.g. `Improve Tracks`. Membership is a query, not rows, so
    /// `collection_tracks` stays empty for these.
    Smart,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub kind: CollectionKind,
    /// Smart collections only: a serialized `TrackFilters`.
    pub rule: Option<String>,
    pub color: Option<String>,
    pub position: i32,
    /// Every sidebar row carries a count, so it is part of the read model
    /// rather than a second round trip per row.
    pub track_count: usize,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionTrack {
    pub collection_id: String,
    pub track_id: String,
    pub position: i32,
    pub added_at: i64,
}

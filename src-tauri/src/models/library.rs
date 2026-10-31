use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedFolder {
    pub id: String,
    pub path: String,
    pub is_active: bool,
    pub track_count: usize,
    pub added_at: i64,
}

/// Progress of the filesystem walk, which is IO-bound and finishes long before
/// the analysis queue does. Kept separate from `AnalysisProgress` for that reason.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub total: usize,
    pub done: usize,
    pub skipped: usize,
    pub failed: usize,
    pub current_path: Option<String>,
    pub is_scanning: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileReport {
    pub seen: usize,
    pub touched: usize,
    pub marked_missing: usize,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanCacheReport {
    pub tracks_removed: usize,
    pub analysis_rows_removed: usize,
    pub peak_files_removed: usize,
    pub bytes_freed: u64,
}

/// A plain folder listing for the "add tracks" flow. Keystone has no pack tree:
/// a DJ library is flat, and the wheel plus collections do the narrowing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolder {
    pub path: String,
    pub name: String,
    pub track_count: usize,
}

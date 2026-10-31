//! The whole settings surface from SPEC.md §3, flat, serialized to
//! `settings.json` behind an `Arc<RwLock<Settings>>`. Not a table: the surface
//! is large but flat and single-user, and a settings read happens on every
//! tempo format.

use serde::{Deserialize, Serialize};

/// Update Tags -> "What to write".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TagWriteContent {
    /// `10A`
    KeyOnly,
    /// `Energy 7`
    EnergyWithWord,
    /// `10A - Energy 7`
    KeyAndEnergyWithWord,
    /// `7`
    EnergyOnly,
    /// `10A - 7`
    KeyAndEnergy,
    /// `10A - 126`
    KeyAndTempo,
}

/// Update Tags -> "Where to write it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TagWritePlacement {
    FrontOfArtist,
    FrontOfTitle,
    EndOfTitle,
    FrontOfComment,
    OverwriteComment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenameMode {
    Never,
    AfterProcessing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenamePattern {
    OriginalKey,
    /// Recommended.
    OriginalKeyTempo,
    KeyOriginal,
    KeyTempoOriginal,
    TempoKeyOriginal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyNotation {
    /// `11A`
    Camelot,
    /// `G♭m`
    Flats,
    /// `F#m`
    Sharps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtraKeyColumn {
    None,
    Flats,
    Sharps,
}

/// Column visibility, one flag per column the spec lists under Display Options.
/// A struct rather than a `Vec<String>` so a typo in a column name is a compile
/// error on both sides of the boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnVisibility {
    pub cover_art: bool,
    pub path: bool,
    pub artist: bool,
    pub title: bool,
    pub tempo: bool,
    pub key_result: bool,
    pub energy: bool,
    pub cue_points: bool,
    pub date_added: bool,
    pub grouping: bool,
    pub genre: bool,
    pub year: bool,
    pub album: bool,
    pub volume: bool,
    pub clipped_peaks: bool,
    pub rating: bool,
    pub bitrate: bool,
    pub comment: bool,
}

impl Default for ColumnVisibility {
    /// The spec's "default visible set", plus Title which the table cannot
    /// usefully hide.
    fn default() -> Self {
        Self {
            cover_art: true,
            path: false,
            artist: true,
            title: true,
            tempo: true,
            key_result: true,
            energy: true,
            cue_points: true,
            date_added: true,
            grouping: false,
            genre: false,
            year: false,
            album: false,
            volume: false,
            clipped_peaks: false,
            rating: true,
            bitrate: false,
            comment: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    // --- Update Tags ------------------------------------------------------
    pub update_tags_enabled: bool,
    pub tag_write_content: TagWriteContent,
    pub tag_write_placement: TagWritePlacement,
    /// The dedicated key field Serato and Traktor read.
    pub update_initial_key_tag: bool,
    pub write_energy_to_grouping: bool,
    pub update_tempo_tag: bool,
    pub keep_existing_tempo: bool,
    /// Camelot is not in TKEY's spec-legal value set (at most 3 characters from
    /// A-G, b, #, m, plus o). Others write it anyway and the DJ apps accept it,
    /// but third-party taggers may mangle it, so this is an explicit opt-in tied
    /// to the existing Key Notation setting rather than a second control.
    pub write_camelot_to_initial_key: bool,

    // --- Export Cue Points ------------------------------------------------
    pub export_serato: bool,
    pub export_traktor: bool,
    pub export_rekordbox: bool,
    /// Set by the spec's `CHANGE` button. Detection is best-effort — Traktor
    /// creates a new root directory on every version update and the root is
    /// user-overridable — so the override is mandatory, not a nicety.
    pub serato_path: Option<String>,
    pub traktor_path: Option<String>,
    pub rekordbox_path: Option<String>,
    pub overwrite_existing_cues: bool,
    /// Traktor's Key column is backed by MUSICAL_KEY, which Traktor may
    /// overwrite on its own re-analysis, and whose numbering is unresolved in
    /// public sources. Default off: write INFO/@KEY (Key Text) instead.
    pub write_traktor_musical_key: bool,

    // --- Rename Files -----------------------------------------------------
    pub rename_mode: RenameMode,
    pub rename_pattern: RenamePattern,

    // --- Key Notation -----------------------------------------------------
    pub key_notation: KeyNotation,
    pub extra_key_column: ExtraKeyColumn,

    // --- Tempo ------------------------------------------------------------
    /// Anything outside the range is doubled or halved rather than reported
    /// wrong. The fold happens at read time from `tempo_raw`, so changing this
    /// never costs a re-analysis.
    pub restrict_tempo_range: bool,
    pub tempo_min: f64,
    pub tempo_max: f64,
    /// 0, 1 or 2. Two decimals are gated on the grid residual being under
    /// 15 ms per track; below that they are false precision.
    pub tempo_decimals: u8,

    // --- Audio Player -----------------------------------------------------
    /// Off is for when other DJ software is running and holds the device.
    pub play_automatically: bool,
    pub start_at_quarter: bool,
    pub cache_waveform_during_analysis: bool,

    // --- Display Options --------------------------------------------------
    pub columns: ColumnVisibility,
    pub pad_camelot_numbers: bool,
    /// Where Keystone points at Capstone.
    pub show_improvable_message: bool,
    pub show_detected_cue_points: bool,
    pub show_detected_energy_levels: bool,

    // --- Engine -----------------------------------------------------------
    /// IO stage width. Separate from the CPU stage: the walk is IO-bound.
    pub scanner_concurrency: usize,
    /// CPU stage width. Defaults to the physical core count at first run.
    pub analysis_concurrency: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            update_tags_enabled: true,
            tag_write_content: TagWriteContent::KeyAndEnergyWithWord,
            tag_write_placement: TagWritePlacement::FrontOfTitle,
            update_initial_key_tag: true,
            write_energy_to_grouping: false,
            update_tempo_tag: false,
            keep_existing_tempo: true,
            write_camelot_to_initial_key: false,

            export_serato: false,
            export_traktor: false,
            export_rekordbox: false,
            serato_path: None,
            traktor_path: None,
            rekordbox_path: None,
            // Dry run by default: the export action shows a count of what would
            // change before anything is written.
            overwrite_existing_cues: false,
            write_traktor_musical_key: false,

            rename_mode: RenameMode::Never,
            rename_pattern: RenamePattern::OriginalKeyTempo,

            key_notation: KeyNotation::Camelot,
            extra_key_column: ExtraKeyColumn::None,

            restrict_tempo_range: true,
            tempo_min: 79.0,
            tempo_max: 192.0,
            tempo_decimals: 0,

            play_automatically: true,
            start_at_quarter: false,
            cache_waveform_during_analysis: true,

            columns: ColumnVisibility::default(),
            pad_camelot_numbers: false,
            show_improvable_message: true,
            show_detected_cue_points: true,
            show_detected_energy_levels: true,

            scanner_concurrency: 8,
            analysis_concurrency: 0,
        }
    }
}

impl Settings {
    /// Resolved CPU stage width: 0 means "decide from the machine".
    pub fn effective_analysis_concurrency(&self) -> usize {
        if self.analysis_concurrency > 0 {
            self.analysis_concurrency
        } else {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        }
    }

    /// The fold range, or the open range when the user turned the restriction
    /// off. Callers fold from `tempo_raw`; they never re-detect.
    pub fn tempo_range(&self) -> (f64, f64) {
        if self.restrict_tempo_range {
            (self.tempo_min, self.tempo_max)
        } else {
            (0.0, f64::INFINITY)
        }
    }
}

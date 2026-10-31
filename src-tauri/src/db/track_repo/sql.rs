//! The two statements with thirty columns between them, plus the `TagField`
//! to column map. Kept apart from the call sites so the writes read as writes.

use crate::models::TagField;

/// Inserted by the walk, before anything is analysed, so a freshly added folder
/// is in the table immediately.
///
/// The DO UPDATE branch mentions no analysis column at all — stronger than a
/// COALESCE, because it cannot be defeated by a zero. The tag and file-fact
/// columns do use COALESCE: a walk that could not read tags offers NULL, and
/// NULL must not blank what a previous pass read. Clearing a tag on purpose
/// goes through `update_tags`, which writes the absence explicitly.
pub(super) const UPSERT_SQL: &str = "\
INSERT INTO tracks (
  id, path, filename, extension, size_bytes, content_hash,
  duration_ms, sample_rate, channels, bit_depth, bitrate_kbps, codec,
  artist, title, album, genre, grouping, comment, year, tag_bpm, tag_key, has_cover_art,
  index_status, date_added, created_at, updated_at, last_seen_at
) VALUES (
  :id, :path, :filename, :extension, :size_bytes, :content_hash,
  :duration_ms, :sample_rate, :channels, :bit_depth, :bitrate_kbps, :codec,
  :artist, :title, :album, :genre, :grouping, :comment, :year, :tag_bpm, :tag_key, :has_cover_art,
  'indexed', :now, :now, :now, :now
)
ON CONFLICT(path) DO UPDATE SET
  filename      = excluded.filename,
  extension     = excluded.extension,
  size_bytes    = COALESCE(excluded.size_bytes, tracks.size_bytes),
  content_hash  = COALESCE(excluded.content_hash, tracks.content_hash),
  duration_ms   = COALESCE(excluded.duration_ms, tracks.duration_ms),
  sample_rate   = COALESCE(excluded.sample_rate, tracks.sample_rate),
  channels      = COALESCE(excluded.channels, tracks.channels),
  bit_depth     = COALESCE(excluded.bit_depth, tracks.bit_depth),
  bitrate_kbps  = COALESCE(excluded.bitrate_kbps, tracks.bitrate_kbps),
  codec         = COALESCE(excluded.codec, tracks.codec),
  artist        = COALESCE(excluded.artist, tracks.artist),
  title         = COALESCE(excluded.title, tracks.title),
  album         = COALESCE(excluded.album, tracks.album),
  genre         = COALESCE(excluded.genre, tracks.genre),
  grouping      = COALESCE(excluded.grouping, tracks.grouping),
  comment       = COALESCE(excluded.comment, tracks.comment),
  year          = COALESCE(excluded.year, tracks.year),
  tag_bpm       = COALESCE(excluded.tag_bpm, tracks.tag_bpm),
  tag_key       = COALESCE(excluded.tag_key, tracks.tag_key),
  has_cover_art = COALESCE(excluded.has_cover_art, tracks.has_cover_art),
  index_status  = 'indexed',
  error_message = NULL,
  updated_at    = excluded.updated_at,
  last_seen_at  = excluded.last_seen_at";

/// A user override survives re-analysis: each guarded column keeps its value
/// when its `*_source` says `user`. Confidences and provenance are rewritten
/// either way, so the badge still tells the truth about what the engine thinks.
pub(super) const APPLY_ANALYSIS_SQL: &str = "\
UPDATE tracks SET
  duration_ms        = :duration_ms,
  tempo_raw          = CASE WHEN tempo_source = 'user' THEN tempo_raw          ELSE :tempo_raw END,
  tempo_display      = CASE WHEN tempo_source = 'user' THEN tempo_display      ELSE :tempo_display END,
  tempo_octave_shift = CASE WHEN tempo_source = 'user' THEN tempo_octave_shift ELSE :octave_shift END,
  tempo_confidence   = :tempo_confidence,
  tempo_residual_ms  = :residual_ms,
  tempo_variable     = :variable,
  tempo_source       = CASE WHEN tempo_source = 'user' THEN 'user' ELSE 'detected' END,
  key_root           = CASE WHEN key_source = 'user' THEN key_root ELSE :key_root END,
  key_mode           = CASE WHEN key_source = 'user' THEN key_mode ELSE :key_mode END,
  camelot            = CASE WHEN key_source = 'user' THEN camelot  ELSE :camelot END,
  key_confidence     = :key_confidence,
  key_alt_root       = :alt_root,
  key_alt_mode       = :alt_mode,
  key_mode_ambiguous = :mode_ambiguous,
  tuning_cents       = :tuning_cents,
  key_source         = CASE WHEN key_source = 'user' THEN 'user' ELSE 'detected' END,
  energy             = CASE WHEN energy_source = 'user' THEN energy ELSE :energy END,
  energy_raw         = :energy_raw,
  energy_source      = CASE WHEN energy_source = 'user' THEN 'user' ELSE 'detected' END,
  peak_db            = :peak_db,
  true_peak_db       = :true_peak_db,
  lufs_integrated    = :lufs,
  clipped_peaks      = :clipped,
  analysis_version     = :analysis_version,
  energy_model_version = :energy_model_version,
  analyzed_at        = :now,
  index_status       = 'indexed',
  error_message      = NULL,
  updated_at         = :now
WHERE id = :id";

/// `TagField` to column. Not a `Display` on the model: these are column names,
/// and a model that knows them invites a caller to build SQL outside this layer.
pub(super) fn tag_column(field: TagField) -> &'static str {
    match field {
        TagField::Artist => "artist",
        TagField::Title => "title",
        TagField::Album => "album",
        TagField::Genre => "genre",
        TagField::Grouping => "grouping",
        TagField::Comment => "comment",
        TagField::Year => "year",
        TagField::Bpm => "tag_bpm",
        TagField::InitialKey => "tag_key",
    }
}

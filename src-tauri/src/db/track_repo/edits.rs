//! Per-field writes: the ones a user action makes, one column at a time.
//!
//! Each stamps its own `*_source`, which is what makes the guard in
//! `apply_analysis` mean something: provenance is recorded at the moment of the
//! edit, not inferred afterwards.

use rusqlite::named_params;

use super::TrackRepository;
use crate::analysis::camelot::Camelot;
use crate::db::mapping::{now, value_source_str};
use crate::error::Result;
use crate::models::{KeyMode, TagSnapshot, ValueSource};

impl TrackRepository {
    pub fn set_rating(&self, track_id: &str, rating: Option<u8>) -> Result<()> {
        self.conn()?.execute(
            "UPDATE tracks SET rating = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![rating, now(), track_id],
        )?;
        Ok(())
    }

    /// Recomputes `camelot` here rather than trusting a caller to pass it: the
    /// badge, the wheel filter and the exported `Tonality` all read that column,
    /// so one place must own the mapping.
    pub fn set_key(
        &self,
        track_id: &str,
        root: u8,
        mode: KeyMode,
        source: ValueSource,
    ) -> Result<()> {
        let camelot = Camelot::from_key(root, mode).label(false);
        self.conn()?.execute(
            "UPDATE tracks SET key_root = ?1, key_mode = ?2, camelot = ?3, key_source = ?4, \
             updated_at = ?5 WHERE id = ?6",
            rusqlite::params![
                root,
                mode.as_str(),
                camelot,
                value_source_str(source),
                now(),
                track_id
            ],
        )?;
        Ok(())
    }

    /// A typed tempo is already what the user wants to see, so raw and display
    /// agree and the octave shift is zero: there is nothing left to fold.
    pub fn set_tempo(&self, track_id: &str, bpm: f64, source: ValueSource) -> Result<()> {
        self.conn()?.execute(
            "UPDATE tracks SET tempo_raw = ?1, tempo_display = ?1, tempo_octave_shift = 0, \
             tempo_source = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![bpm, value_source_str(source), now(), track_id],
        )?;
        Ok(())
    }

    pub fn set_energy(&self, track_id: &str, energy: u8, source: ValueSource) -> Result<()> {
        self.conn()?.execute(
            "UPDATE tracks SET energy = ?1, energy_source = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![energy, value_source_str(source), now(), track_id],
        )?;
        Ok(())
    }

    /// Mirrors what was just read from or written to the file's tags, and
    /// touches nothing else: a tag edit must not cost a re-analysis, so it
    /// leaves `content_hash` and every analysis column alone.
    pub fn update_tags(&self, track_id: &str, snapshot: &TagSnapshot) -> Result<()> {
        self.conn()?.execute(
            "UPDATE tracks SET artist = :artist, title = :title, album = :album, genre = :genre, \
             grouping = :grouping, comment = :comment, year = :year, tag_bpm = :bpm, \
             tag_key = :key, has_cover_art = :cover, updated_at = :now WHERE id = :id",
            named_params! {
                ":artist": snapshot.artist,
                ":title": snapshot.title,
                ":album": snapshot.album,
                ":genre": snapshot.genre,
                ":grouping": snapshot.grouping,
                ":comment": snapshot.comment,
                ":year": snapshot.year,
                ":bpm": snapshot.bpm,
                ":key": snapshot.initial_key,
                ":cover": snapshot.has_cover_art,
                ":now": now(),
                ":id": track_id,
            },
        )?;
        Ok(())
    }

    /// After a rename. `original_filename` is recorded once and never
    /// overwritten by a second rename, which is what makes the inverse strip
    /// exact instead of a regex guess over a name Keystone already edited.
    pub fn update_path(
        &self,
        track_id: &str,
        new_path: &str,
        original_filename: Option<&str>,
        pattern: Option<&str>,
    ) -> Result<()> {
        let filename = std::path::Path::new(new_path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| new_path.to_string());
        self.conn()?.execute(
            "UPDATE tracks SET path = ?1, filename = ?2, \
             original_filename = COALESCE(original_filename, ?3), rename_pattern = ?4, \
             updated_at = ?5 WHERE id = ?6",
            rusqlite::params![new_path, filename, original_filename, pattern, now(), track_id],
        )?;
        Ok(())
    }

    pub fn increment_play_count(&self, track_id: &str) -> Result<()> {
        self.conn()?.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played = ?1 WHERE id = ?2",
            rusqlite::params![now(), track_id],
        )?;
        Ok(())
    }

    pub fn delete(&self, track_id: &str) -> Result<()> {
        // Cues, beatgrid and regions go with it through ON DELETE CASCADE,
        // which only fires because connection.rs turns foreign keys on.
        self.conn()?
            .execute("DELETE FROM tracks WHERE id = ?1", [track_id])?;
        Ok(())
    }
}

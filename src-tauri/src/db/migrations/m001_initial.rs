//! Migration 001, as ENGINE.md §5 specifies it.
//!
//! FTS5 is here from the start rather than retrofitted. Stack retrofitted its
//! search index in later migrations, the rebuild left the index empty, and that
//! is the entire reason it carries a multi-second repair pass on every launch.

pub const SQL: &str = r#"
CREATE TABLE tracks (
  id                   TEXT PRIMARY KEY,          -- xxh3 of the path
  path                 TEXT NOT NULL UNIQUE,
  filename             TEXT NOT NULL,
  original_filename    TEXT,                      -- pre-rename, for the exact strip
  rename_pattern       TEXT,                      -- which pattern was applied
  extension            TEXT NOT NULL,
  content_hash         TEXT,                      -- xxh3 of the AUDIO REGION

  -- file facts
  size_bytes           INTEGER,
  duration_ms          INTEGER,
  sample_rate          INTEGER,
  channels             INTEGER,
  bit_depth            INTEGER,
  bitrate_kbps         INTEGER,
  codec                TEXT,

  -- tags as read from the file
  artist               TEXT,
  title                TEXT,
  album                TEXT,
  genre                TEXT,
  grouping             TEXT,
  comment              TEXT,
  year                 INTEGER,
  tag_bpm              REAL,
  tag_key              TEXT,
  has_cover_art        INTEGER DEFAULT 0,

  -- analysis: tempo
  tempo_raw            REAL,                      -- as detected, unfolded
  tempo_display        REAL,                      -- folded into the user's range
  tempo_octave_shift   INTEGER,                   -- -1 / 0 / +1
  tempo_confidence     REAL,
  tempo_residual_ms    REAL,                      -- grid fit sd; <15 gates two decimals
  tempo_variable       INTEGER DEFAULT 0,

  -- analysis: key
  key_root             INTEGER,                   -- 0-11, 0 = C
  key_mode             TEXT,                      -- 'major' | 'minor'
  camelot              TEXT,                      -- '8A'
  key_confidence       REAL,
  key_alt_root         INTEGER,
  key_alt_mode         TEXT,
  key_mode_ambiguous   INTEGER DEFAULT 0,
  tuning_cents         REAL,

  -- analysis: energy and levels
  energy               INTEGER,                   -- 1-10
  energy_raw           REAL,
  peak_db              REAL,
  true_peak_db         REAL,
  lufs_integrated      REAL,
  clipped_peaks        INTEGER,

  -- user data
  rating               INTEGER,                   -- 0-5
  date_added           INTEGER NOT NULL,
  last_played          INTEGER,
  play_count           INTEGER DEFAULT 0,

  -- provenance and lifecycle
  key_source           TEXT,                      -- 'detected' | 'user' | 'tag'
  tempo_source         TEXT,
  energy_source        TEXT,
  analysis_version     INTEGER DEFAULT 0,
  energy_model_version INTEGER DEFAULT 0,
  analyzed_at          INTEGER,
  index_status         TEXT DEFAULT 'pending',    -- pending|indexed|missing|error
  error_message        TEXT,

  created_at           INTEGER NOT NULL,
  updated_at           INTEGER NOT NULL,
  last_seen_at         INTEGER NOT NULL
);

-- Results keyed by CONTENT, so a moved, renamed or duplicated file costs no DSP.
CREATE TABLE analysis_cache (
  content_hash         TEXT NOT NULL,
  analysis_version     INTEGER NOT NULL,
  energy_model_version INTEGER NOT NULL,
  payload              TEXT NOT NULL,             -- JSON: key, tempo, energy, cues, regions, beatgrid
  created_at           INTEGER NOT NULL,
  PRIMARY KEY (content_hash, analysis_version)
);

CREATE TABLE cue_points (
  id            TEXT PRIMARY KEY,
  track_id      TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  slot          INTEGER NOT NULL,                 -- 0-7, the eight CUE markers
  kind          TEXT NOT NULL,                    -- intro|first_beat|breakdown|build|drop|outro|loop|user
  position_ms   REAL NOT NULL,                    -- REAL: rekordbox 1/150 s frames, Serato ms, Traktor ms float
  end_ms        REAL,                             -- loops only
  bar_index     INTEGER,
  phrase_level  INTEGER,                          -- 4 | 8 | 16 | 32
  label         TEXT,
  color         TEXT,
  confidence    REAL,
  source        TEXT NOT NULL DEFAULT 'detected', -- 'detected' | 'user'
  created_at    INTEGER NOT NULL,
  UNIQUE (track_id, slot)
);

-- One row per tempo segment, mirroring rekordbox's TEMPO element so export is musically correct.
CREATE TABLE beatgrid (
  id            TEXT PRIMARY KEY,
  track_id      TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  position_ms   REAL NOT NULL,                    -- Inizio
  bpm           REAL NOT NULL,
  beats_per_bar INTEGER NOT NULL DEFAULT 4,       -- Metro
  beat_in_bar   INTEGER NOT NULL DEFAULT 1        -- Battito
);

-- The read model the deferred deck view needs. Costs nothing now; a migration later.
CREATE TABLE track_regions (
  id            TEXT PRIMARY KEY,
  track_id      TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  start_ms      REAL NOT NULL,
  end_ms        REAL NOT NULL,
  bar_index     INTEGER,
  bars          INTEGER,
  kind          TEXT,                             -- intro|build|drop|breakdown|outro|unknown
  energy        INTEGER,
  key_root      INTEGER,
  key_mode      TEXT
);

CREATE TABLE collections (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'user',       -- 'user' | 'smart'
  rule        TEXT,                               -- smart only, e.g. Improve Tracks
  color       TEXT,
  position    INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL
);

CREATE TABLE collection_tracks (
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  track_id      TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  position      INTEGER NOT NULL DEFAULT 0,
  added_at      INTEGER NOT NULL,
  PRIMARY KEY (collection_id, track_id)
);

-- The user's root folders. Not in the ENGINE.md listing, but SPEC.md's
-- ADD TRACKS button and the library commands need somewhere to persist them.
CREATE TABLE watched_folders (
  id         TEXT PRIMARY KEY,
  path       TEXT NOT NULL UNIQUE,
  is_active  INTEGER NOT NULL DEFAULT 1,
  added_at   INTEGER NOT NULL
);

-- What makes RE-TAG EXISTING FILES a shippable button rather than a terrifying one.
CREATE TABLE tag_backup (
  id          TEXT PRIMARY KEY,
  track_id    TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  container   TEXT NOT NULL,                      -- id3v2 | ilst | vorbis | ape
  raw         BLOB NOT NULL,                      -- the replaced tag bytes
  written_at  INTEGER NOT NULL
);

-- Per-file transaction log, so an interrupted 5,000-track export is reportable and resumable.
CREATE TABLE export_log (
  id          TEXT PRIMARY KEY,
  track_id    TEXT NOT NULL,
  target      TEXT NOT NULL,                      -- rekordbox | traktor | serato
  status      TEXT NOT NULL,                      -- planned | written | verified | failed | skipped
  detail      TEXT,
  run_id      TEXT NOT NULL,
  written_at  INTEGER NOT NULL
);

-- Journal-first renames: the row exists before fs::rename and is cleared after the UPDATE.
CREATE TABLE file_ops (
  id          TEXT PRIMARY KEY,
  track_id    TEXT NOT NULL,
  op          TEXT NOT NULL,                      -- 'rename'
  from_path   TEXT NOT NULL,
  to_path     TEXT NOT NULL,
  state       TEXT NOT NULL,                      -- intended | renamed | done
  created_at  INTEGER NOT NULL
);

CREATE VIRTUAL TABLE tracks_fts USING fts5(
  artist, title, album, genre, grouping, comment, filename,
  content='tracks', content_rowid='rowid', tokenize='unicode61'
);
CREATE VIRTUAL TABLE tracks_vocab USING fts5vocab(tracks_fts, 'row');

-- An external-content FTS5 table does not follow its content table on its own.
-- These three triggers are what keep the index from going stale, which is the
-- failure that forces a rebuild-and-backfill pass later.
CREATE TRIGGER tracks_fts_insert AFTER INSERT ON tracks BEGIN
  INSERT INTO tracks_fts(rowid, artist, title, album, genre, grouping, comment, filename)
  VALUES (new.rowid, new.artist, new.title, new.album, new.genre, new.grouping, new.comment, new.filename);
END;
CREATE TRIGGER tracks_fts_delete AFTER DELETE ON tracks BEGIN
  INSERT INTO tracks_fts(tracks_fts, rowid, artist, title, album, genre, grouping, comment, filename)
  VALUES ('delete', old.rowid, old.artist, old.title, old.album, old.genre, old.grouping, old.comment, old.filename);
END;
CREATE TRIGGER tracks_fts_update AFTER UPDATE ON tracks BEGIN
  INSERT INTO tracks_fts(tracks_fts, rowid, artist, title, album, genre, grouping, comment, filename)
  VALUES ('delete', old.rowid, old.artist, old.title, old.album, old.genre, old.grouping, old.comment, old.filename);
  INSERT INTO tracks_fts(rowid, artist, title, album, genre, grouping, comment, filename)
  VALUES (new.rowid, new.artist, new.title, new.album, new.genre, new.grouping, new.comment, new.filename);
END;

-- Always-present leading filters are the Camelot selection and the collection,
-- so composites lead with those. Every partial index repeats the WHERE clause
-- verbatim: a predicate spelled differently in the query than in the index
-- makes SQLite fetch every row back to re-check it.
CREATE INDEX idx_tracks_camelot        ON tracks(camelot)                WHERE index_status != 'missing';
CREATE INDEX idx_tracks_camelot_energy ON tracks(camelot, energy)        WHERE index_status != 'missing';
CREATE INDEX idx_tracks_camelot_tempo  ON tracks(camelot, tempo_display) WHERE index_status != 'missing';
CREATE INDEX idx_tracks_energy         ON tracks(energy)                 WHERE index_status != 'missing';
CREATE INDEX idx_tracks_tempo          ON tracks(tempo_display)          WHERE index_status != 'missing';
CREATE INDEX idx_tracks_genre          ON tracks(genre)                  WHERE index_status != 'missing';
CREATE INDEX idx_tracks_rating         ON tracks(rating)                 WHERE index_status != 'missing';
CREATE INDEX idx_tracks_path           ON tracks(path);
CREATE INDEX idx_tracks_content_hash   ON tracks(content_hash);
CREATE INDEX idx_tracks_index_status   ON tracks(index_status);
CREATE INDEX idx_tracks_last_seen      ON tracks(last_seen_at);
CREATE INDEX idx_tracks_analysis_ver   ON tracks(analysis_version)       WHERE index_status != 'missing';
-- The default sort column WITH ITS COLLATION. Without the collation in the
-- index, every page load sorts the whole table through a temp B-tree carrying
-- all projected columns.
CREATE INDEX idx_tracks_artist_nocase  ON tracks(artist COLLATE NOCASE);
CREATE INDEX idx_tracks_date_added     ON tracks(date_added);
CREATE INDEX idx_coll_tracks_pos       ON collection_tracks(collection_id, position);
CREATE INDEX idx_cues_track_slot       ON cue_points(track_id, slot);
CREATE INDEX idx_regions_track         ON track_regions(track_id, start_ms);
CREATE INDEX idx_export_log_run        ON export_log(run_id, status);
CREATE INDEX idx_file_ops_state        ON file_ops(state);
"#;

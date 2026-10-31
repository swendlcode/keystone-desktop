/**
 * Mirrors `src-tauri/src/models/settings.rs` — SPEC.md §3, flat.
 *
 * Lives in settings.json behind an Arc<RwLock<_>>, not in SQLite: the surface
 * is large but flat and single-user, and a settings read happens on every
 * tempo format.
 *
 * Two things the user picks are deliberately NOT here, because the backend
 * does not store them: the theme and the player volume. Both have to be
 * readable before any IPC completes — the theme to avoid a dark-then-light
 * flash on first paint, the volume so the first track does not play at full
 * output — so they are persisted in localStorage by `settingsStore` and
 * `playerStore`. Adding them here later means deciding which copy wins.
 */

/** Update Tags -> "What to write". */
export type TagWriteContent =
  | 'keyOnly' //              10A
  | 'energyWithWord' //       Energy 7
  | 'keyAndEnergyWithWord' // 10A - Energy 7
  | 'energyOnly' //           7
  | 'keyAndEnergy' //         10A - 7
  | 'keyAndTempo'; //         10A - 126

/** Update Tags -> "Where to write it". */
export type TagWritePlacement =
  | 'frontOfArtist'
  | 'frontOfTitle'
  | 'endOfTitle'
  | 'frontOfComment'
  | 'overwriteComment';

export type RenameMode = 'never' | 'afterProcessing';

export type RenamePattern =
  | 'originalKey' //       Original - Key
  | 'originalKeyTempo' //  Original - Key - Tempo   (recommended)
  | 'keyOriginal' //       Key - Original
  | 'keyTempoOriginal' //  Key - Tempo - Original
  | 'tempoKeyOriginal'; // Tempo - Key - Original

export type KeyNotation =
  | 'camelot' //  11A
  | 'flats' //    G♭m
  | 'sharps'; //  F#m

export type ExtraKeyColumn = 'none' | 'flats' | 'sharps';

/** 0, 1 or 2 places. Two are gated per track on the grid residual, not here. */
export type TempoDecimals = 0 | 1 | 2;

/**
 * Column visibility: one flag per column, not a list of names, so a typo is a
 * compile error on both sides of the boundary.
 */
export interface ColumnVisibility {
  coverArt: boolean;
  path: boolean;
  artist: boolean;
  title: boolean;
  tempo: boolean;
  keyResult: boolean;
  energy: boolean;
  cuePoints: boolean;
  dateAdded: boolean;
  grouping: boolean;
  genre: boolean;
  year: boolean;
  album: boolean;
  volume: boolean;
  clippedPeaks: boolean;
  rating: boolean;
  bitrate: boolean;
  comment: boolean;
}

/** A column key, derived from the struct so the two cannot drift. */
export type TrackColumn = keyof ColumnVisibility;

/** Column order for the table header. Visibility is a separate question. */
export const COLUMN_ORDER: TrackColumn[] = [
  'coverArt',
  'artist',
  'tempo',
  'keyResult',
  'title',
  'energy',
  'cuePoints',
  'dateAdded',
  'rating',
  'path',
  'grouping',
  'genre',
  'year',
  'album',
  'volume',
  'clippedPeaks',
  'bitrate',
  'comment',
];

/** Header labels, uppercased by `.label` in CSS rather than here. */
export const COLUMN_LABELS: Record<TrackColumn, string> = {
  coverArt: 'Cover art',
  path: 'Path',
  artist: 'Artist',
  title: 'Title',
  tempo: 'Tempo',
  keyResult: 'Key result',
  energy: 'Energy',
  cuePoints: 'Cue points',
  dateAdded: 'Date added',
  grouping: 'Grouping',
  genre: 'Genre',
  year: 'Year',
  album: 'Album',
  volume: 'Volume',
  clippedPeaks: 'Clipped peaks',
  rating: 'Rating',
  bitrate: 'Bitrate',
  comment: 'Comment',
};

export interface Settings {
  // ── Update Tags ──────────────────────────────────────────────────────────
  updateTagsEnabled: boolean;
  tagWriteContent: TagWriteContent;
  tagWritePlacement: TagWritePlacement;
  /** The dedicated key field Serato and Traktor read. */
  updateInitialKeyTag: boolean;
  writeEnergyToGrouping: boolean;
  updateTempoTag: boolean;
  keepExistingTempo: boolean;
  /** Camelot is not in TKEY's spec-legal value set, so this is an explicit
      opt-in tied to `keyNotation` rather than a second control. */
  writeCamelotToInitialKey: boolean;

  // ── Export Cue Points ────────────────────────────────────────────────────
  exportSerato: boolean;
  exportTraktor: boolean;
  exportRekordbox: boolean;
  /** Set by the spec's CHANGE button. Detection is best-effort, so the
      override is mandatory rather than a nicety. */
  seratoPath: string | null;
  traktorPath: string | null;
  rekordboxPath: string | null;
  overwriteExistingCues: boolean;
  /** Off by default: Traktor may overwrite MUSICAL_KEY on its own
      re-analysis, and its numbering is unresolved in public sources. */
  writeTraktorMusicalKey: boolean;

  // ── Rename Files ─────────────────────────────────────────────────────────
  renameMode: RenameMode;
  renamePattern: RenamePattern;

  // ── Key Notation ─────────────────────────────────────────────────────────
  keyNotation: KeyNotation;
  extraKeyColumn: ExtraKeyColumn;

  // ── Tempo ────────────────────────────────────────────────────────────────
  restrictTempoRange: boolean;
  tempoMin: number;
  tempoMax: number;
  tempoDecimals: TempoDecimals;

  // ── Audio Player ─────────────────────────────────────────────────────────
  playAutomatically: boolean;
  startAtQuarter: boolean;
  cacheWaveformDuringAnalysis: boolean;

  // ── Display Options ──────────────────────────────────────────────────────
  columns: ColumnVisibility;
  padCamelotNumbers: boolean;
  /** Where Keystone points at Capstone. */
  showImprovableMessage: boolean;
  showDetectedCuePoints: boolean;
  showDetectedEnergyLevels: boolean;

  // ── Engine ───────────────────────────────────────────────────────────────
  scannerConcurrency: number;
  /** 0 means "decide from the machine". */
  analysisConcurrency: number;
}

/** Client-only, localStorage-backed. See the note at the top of this file. */
export type Theme = 'dark' | 'light';

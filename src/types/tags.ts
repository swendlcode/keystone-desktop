/** Mirrors `src-tauri/src/models/tags.rs`. */

/**
 * The container a file's tags live in.
 *
 * Dispatching on this rather than on a generic tag type is load-bearing:
 * lofty's generic `Tag` drops anything without a mapping, so one bulk replace
 * over it would silently delete every Serato GEOB blob, Traktor blob and
 * ReplayGain frame in the library.
 */
export type Container = 'id3v2' | 'ilst' | 'vorbisComments' | 'ape' | 'riff' | 'aiff';

/** Read from the file, not from the database: the file owns its tags. */
export interface TagSnapshot {
  artist: string | null;
  title: string | null;
  album: string | null;
  genre: string | null;
  grouping: string | null;
  comment: string | null;
  year: number | null;
  /** ID3v2 has no decimal BPM frame, so the decimal tempo the table shows
      always comes from our own database, never from a tag round trip. */
  bpm: number | null;
  initialKey: string | null;
  hasCoverArt: boolean;
}

export type TagField =
  | 'artist'
  | 'title'
  | 'album'
  | 'genre'
  | 'grouping'
  | 'comment'
  | 'year'
  | 'bpm'
  | 'initialKey';

/**
 * A set of field writes.
 *
 * Explicit pairs rather than a struct of optionals so "clear this field" and
 * "leave it alone" cannot be confused at a call site: `[field, null]` clears.
 */
export interface TagEdit {
  fields: Array<[TagField, string | null]>;
}

export interface CoverArt {
  mime: string;
  /** Base64, rendered as a data URL — embedded art never goes through the
      asset protocol, which addresses files rather than frames. */
  dataBase64: string;
}

export interface TagWriteFailure {
  trackId: string;
  path: string;
  reason: string;
}

export interface TagWriteReport {
  runId: string;
  written: number;
  verified: number;
  skipped: number;
  failed: number;
  failures: TagWriteFailure[];
}

export interface TagBackupEntry {
  id: string;
  trackId: string;
  container: Container;
  sizeBytes: number;
  writtenAt: number;
}

/**
 * What a rename would do, shown before anything moves.
 *
 * Renaming is the only irreversible operation in the app and one setting runs
 * it automatically after processing, so the plan is a first-class object rather
 * than a log line.
 */
export interface RenamePlan {
  items: RenamePlanItem[];
}

export interface RenamePlanItem {
  trackId: string;
  fromPath: string;
  toPath: string;
  /** Set when the item will not be renamed, with the reason: already correct
      (idempotence), a collision on a different file, an illegal Windows
      component, or no key/tempo to write. */
  skipped: string | null;
}

export interface RenameReport {
  renamed: number;
  skipped: number;
  failed: number;
  failures: TagWriteFailure[];
}

/** The fields SPEC.md §2's Global Replace runs over. */
export const REPLACE_FIELDS: TagField[] = [
  'artist',
  'title',
  'initialKey',
  'genre',
  'album',
  'grouping',
  'comment',
];

/**
 * Mirrors `src-tauri/src/models/track.rs`.
 *
 * Every struct there carries `#[serde(rename_all = "camelCase")]`, so these
 * names are the wire format, not a convenience. Nullable columns are `| null`
 * rather than optional: an unanalysed track has `camelot: null`, and the table
 * has to render that cell empty rather than crash on `undefined`.
 */

export type KeyMode = 'major' | 'minor';

/** Where a value came from. A user override must survive a re-analysis. */
export type ValueSource = 'detected' | 'user' | 'tag';

export type IndexStatus = 'pending' | 'indexed' | 'missing' | 'error';

/** One library row, exactly as the table renders it. */
export interface Track {
  /** xxh3 of the path. Per-path identity: duplicates keep their own rows. */
  id: string;
  path: string;
  filename: string;
  /** The name before Keystone renamed it — what makes the strip exact. */
  originalFilename: string | null;
  renamePattern: string | null;
  extension: string;
  /** xxh3 of the AUDIO REGION, never the file head, where ID3 lives. */
  contentHash: string | null;

  sizeBytes: number | null;
  durationMs: number | null;
  sampleRate: number | null;
  channels: number | null;
  bitDepth: number | null;
  bitrateKbps: number | null;
  codec: string | null;

  artist: string | null;
  title: string | null;
  album: string | null;
  genre: string | null;
  grouping: string | null;
  comment: string | null;
  year: number | null;
  tagBpm: number | null;
  tagKey: string | null;
  hasCoverArt: boolean;

  /** As detected, unfolded. Kept apart from `tempoDisplay` so changing the
      Tempo range setting re-folds from the database with no re-analysis. */
  tempoRaw: number | null;
  tempoDisplay: number | null;
  tempoOctaveShift: number | null;
  tempoConfidence: number | null;
  /** Grid-fit residual sd. Under 15 ms is what licenses two decimals. */
  tempoResidualMs: number | null;
  tempoVariable: boolean;

  /** 0-11, 0 = C. */
  keyRoot: number | null;
  keyMode: KeyMode | null;
  camelot: string | null;
  keyConfidence: number | null;
  keyAltRoot: number | null;
  keyAltMode: KeyMode | null;
  /** Show the relative pair (`8A/8B`) rather than committing to a letter. */
  keyModeAmbiguous: boolean;
  tuningCents: number | null;

  energy: number | null;
  energyRaw: number | null;
  peakDb: number | null;
  truePeakDb: number | null;
  lufsIntegrated: number | null;
  clippedPeaks: number | null;

  rating: number | null;
  dateAdded: number;
  lastPlayed: number | null;
  playCount: number;

  keySource: ValueSource | null;
  tempoSource: ValueSource | null;
  energySource: ValueSource | null;
  analysisVersion: number;
  energyModelVersion: number;
  analyzedAt: number | null;
  indexStatus: IndexStatus;
  errorMessage: string | null;

  createdAt: number;
  updatedAt: number;
  lastSeenAt: number;
}

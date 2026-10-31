import type { KeyMode } from './track';
import type { BeatgridMarker, DetectedCue } from './cue';

/** Mirrors `src-tauri/src/models/analysis.rs`. */

export interface ChunkKey {
  startMs: number;
  endMs: number;
  root: number;
  mode: KeyMode;
  confidence: number;
}

export interface KeyResult {
  /** 0-11, 0 = C. */
  root: number;
  mode: KeyMode;
  camelot: string;
  confidence: number;
  altRoot: number | null;
  altMode: KeyMode | null;
  /** Voter disagreement on mode. Drives the relative-pair display and the
      `Improve Tracks` collection. */
  modeAmbiguous: boolean;
  tuningCents: number;
  /** Per-15-s-chunk keys. Stored, not displayed in v1. */
  chunkKeys: ChunkKey[];
}

export interface TempoResult {
  /** Unfolded. Folding happens at read time against the user's range. */
  rawBpm: number;
  octaveShift: number;
  confidence: number;
  residualMs: number;
  variable: boolean;
  beatsMs: number[];
  downbeatPhase: number;
  downbeatConfidence: number;
  beatsPerBar: number;
  phraseBars: number;
}

export interface EnergyFeatures {
  onsetRate: number;
  fluxMean: number;
  lowRatio: number;
  highRatio: number;
  centroid: number;
  percRatio: number;
  dynamicSpan: number;
}

export interface EnergyResult {
  /** 1-10. */
  energy: number;
  raw: number;
  /** The frozen constants' version. Energy numbers get written into the user's
      file tags, so changing them is a migration, not a tweak. */
  modelVersion: number;
  features: EnergyFeatures;
}

export type RegionKind =
  | 'intro'
  | 'build'
  | 'drop'
  | 'breakdown'
  | 'outro'
  | 'unknown';

export interface TrackRegion {
  id: string;
  trackId: string;
  startMs: number;
  endMs: number;
  barIndex: number | null;
  bars: number | null;
  kind: RegionKind;
  /** Computed with the identical frozen constants as the track energy. */
  energy: number | null;
  keyRoot: number | null;
  keyMode: KeyMode | null;
}

export interface DetectedRegion {
  startMs: number;
  endMs: number;
  barIndex: number | null;
  bars: number | null;
  kind: RegionKind;
  energy: number | null;
  keyRoot: number | null;
  keyMode: KeyMode | null;
}

export interface LevelsResult {
  peakDb: number;
  truePeakDb: number;
  lufsIntegrated: number;
  clippedPeaks: number;
}

/** Everything one pass over one file produced — the `analysis_cache` payload. */
export interface AnalysisResult {
  contentHash: string;
  analysisVersion: number;
  energyModelVersion: number;
  durationMs: number;
  key: KeyResult;
  tempo: TempoResult;
  energy: EnergyResult;
  levels: LevelsResult;
  cues: DetectedCue[];
  regions: DetectedRegion[];
  beatgrid: BeatgridMarker[];
}

export type QueueState = 'idle' | 'running' | 'paused';

/**
 * The sidebar's Analysis Queue object.
 *
 * `Empty` in the UI is `state === 'idle'` with `total === 0` — a state, not a
 * percentage, which is why there is a separate queue-finished event.
 */
export interface AnalysisProgress {
  state: QueueState;
  queued: number;
  running: number;
  done: number;
  failed: number;
  /** Cache hits: a moved, renamed or duplicate file that cost no DSP. This is
      the answer to "why was the re-scan instant", so it is worth showing. */
  fromCache: number;
  total: number;
  currentPath: string | null;
  /** Null until the pipeline has been measured on Windows. */
  etaMs: number | null;
}

/** One cached zoom level of the waveform. */
export interface WaveformLevel {
  samplesPerBucket: number;
  min: number[];
  max: number[];
}

/**
 * The peak pyramid, served from disk keyed by `contentHash` — never from the
 * `tracks` row, which the virtualized table selects on every page.
 */
export interface Waveform {
  contentHash: string;
  durationMs: number;
  sampleRate: number;
  levels: WaveformLevel[];
}

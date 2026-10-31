/** Mirrors `src-tauri/src/models/cue.rs`. */

/**
 * The role S11 assigned. Stored rather than derived from the slot number,
 * because the label is worth more to a DJ than an unlabelled ranking.
 *
 * Note the wire values are snake_case here (`first_beat`), not camelCase: the
 * Rust enum uses `rename_all = "snake_case"` so the string matches the
 * `cue_points.kind` column it is written to.
 */
export type CueKind =
  | 'intro'
  | 'first_beat'
  | 'breakdown'
  | 'build'
  | 'drop'
  | 'outro'
  | 'loop'
  | 'user';

export type CueSource = 'detected' | 'user';

/** 4 | 8 | 16 | 32 — the bar level a boundary was retained at. */
export type PhraseLevel = 4 | 8 | 16 | 32;

export interface CuePoint {
  id: string;
  trackId: string;
  /** 0-7. Unique per track, which maps CUE 1..8 to hot cues A-H one-to-one. */
  slot: number;
  kind: CueKind;
  /**
   * Milliseconds as a float, not an integer: rekordbox stores 1/150 s frames
   * and Traktor a float, so rounding here loses positions on the way out.
   */
  positionMs: number;
  /** Loops only. */
  endMs: number | null;
  barIndex: number | null;
  phraseLevel: PhraseLevel | null;
  label: string | null;
  color: string | null;
  confidence: number | null;
  source: CueSource;
  createdAt: number;
}

/** What S11 produces, before it is a row. */
export interface DetectedCue {
  slot: number;
  kind: CueKind;
  positionMs: number;
  endMs: number | null;
  barIndex: number | null;
  phraseLevel: PhraseLevel | null;
  label: string | null;
  confidence: number;
}

/** One tempo segment, mirroring rekordbox's TEMPO element. */
export interface BeatgridMarker {
  id: string;
  trackId: string;
  positionMs: number;
  bpm: number;
  beatsPerBar: number;
  beatInBar: number;
}

/**
 * A cue the user is adding or dragging.
 *
 * Separate from `CuePoint` because the frontend never invents ids or
 * timestamps — the backend owns both.
 */
export interface CueEdit {
  trackId: string;
  slot: number;
  positionMs: number;
  endMs: number | null;
  kind: CueKind | null;
  label: string | null;
  color: string | null;
}

/** Human label for a detected role, for the marker drawn on the waveform. */
export const CUE_KIND_LABELS: Record<CueKind, string> = {
  intro: 'Intro',
  first_beat: 'First beat',
  breakdown: 'Breakdown',
  build: 'Build',
  drop: 'Drop',
  outro: 'Outro',
  loop: 'Loop',
  user: 'Cue',
};

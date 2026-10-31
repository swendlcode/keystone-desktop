import type { Track } from './track';

/**
 * Client-side player state. Not an IPC type: the backend only ever hands back
 * a path for `convertFileSrc`, and decoding happens in the webview.
 */
export interface PlayerState {
  currentTrack: Track | null;
  isPlaying: boolean;
  /** Seconds. */
  currentTime: number;
  duration: number;
  volume: number;
}

/** Waveform zoom steps, coarsest first. Index into `Waveform.levels`. */
export const ZOOM_STEPS = [0, 1, 2, 3, 4] as const;
export type ZoomStep = (typeof ZOOM_STEPS)[number];

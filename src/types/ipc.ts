import type { AnalysisProgress } from './analysis';
import type { ScanProgress } from './library';
import type { Track } from './track';

/**
 * Backend -> frontend event names, mirroring `src-tauri/src/events.rs`.
 *
 * Listened in `useAnalysis` and `useLibrary`. The `keystone://` prefix keeps a
 * sibling app's listener from picking these up.
 */
export const KeystoneEvent = {
  /** Emitted on a timer while the queue runs, not per track: at 8 workers a
      per-track emit floods the webview. */
  analysisProgress: 'keystone://analysis-progress',
  /** One row changed — analysis landed, a tag was written, a rating moved.
      Carries the whole row so the table patches in place without a refetch. */
  trackUpdated: 'keystone://track-updated',
  /** The queue drained. Distinct from progress reaching 100%, because the
      sidebar's `Empty` is a state rather than a percentage. */
  queueFinished: 'keystone://queue-finished',
  scanProgress: 'keystone://scan-progress',
  reconcileComplete: 'keystone://reconcile-complete',
  waveformReady: 'keystone://waveform-ready',
  exportProgress: 'keystone://export-progress',
  /** A re-tag or rename run. Same shape as export, different verb. */
  tagProgress: 'keystone://tag-progress',
} as const;

export type KeystoneEventName =
  (typeof KeystoneEvent)[keyof typeof KeystoneEvent];

export interface AnalysisProgressEvent {
  progress: AnalysisProgress;
}

export interface TrackUpdatedEvent {
  track: Track;
}

export interface QueueFinishedEvent {
  analyzed: number;
  failed: number;
  fromCache: number;
  elapsedMs: number;
}

export interface ScanProgressEvent {
  progress: ScanProgress;
}

export interface WaveformReadyEvent {
  trackId: string;
  contentHash: string;
}

/** Shared by the export and re-tag runs, keyed by `runId` so an interrupted
    5,000-track run is reportable and resumable. */
export interface FileProgressEvent {
  runId: string;
  done: number;
  total: number;
  failed: number;
  currentPath: string | null;
}

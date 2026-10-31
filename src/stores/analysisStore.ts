import { create } from 'zustand';
import type { AnalysisProgress } from '../types';

/**
 * Live analysis-queue state, fed by the `analysis-progress` event.
 *
 * This is client state even though it originates in Rust: it arrives by push
 * on a timer, not by query, so react-query has nothing to key it on and every
 * tick would be a cache write for a number that is only ever displayed.
 *
 * There is deliberately no list of failures here. A failed track carries its
 * own `errorMessage` on the row, which arrives with `track-updated`, so the
 * list is a query over the library rather than a second copy in memory that
 * grows without bound on a folder of 20,000 unreadable files.
 */
interface AnalysisStore {
  progress: AnalysisProgress | null;
  /** The last finished run, so the sidebar can say what happened. */
  lastRun: {
    analyzed: number;
    failed: number;
    fromCache: number;
    elapsedMs: number;
  } | null;

  setProgress: (progress: AnalysisProgress) => void;
  setLastRun: (run: AnalysisStore['lastRun']) => void;
}

export const useAnalysisStore = create<AnalysisStore>((set) => ({
  progress: null,
  lastRun: null,
  setProgress: (progress) => set({ progress }),
  setLastRun: (lastRun) => set({ lastRun }),
}));

/** `Empty` in the sidebar: idle with nothing waiting and nothing in flight. */
export function queueIsEmpty(progress: AnalysisProgress | null): boolean {
  if (!progress) return true;
  return (
    progress.state === 'idle' && progress.queued === 0 && progress.running === 0
  );
}

/** 0-1, or null when there is nothing to divide by — draw indeterminate. */
export function queueFraction(progress: AnalysisProgress | null): number | null {
  if (!progress || progress.total === 0) return null;
  return (progress.done + progress.failed) / progress.total;
}

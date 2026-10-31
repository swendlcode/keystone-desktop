/** Mirrors `src-tauri/src/models/library.rs`. */

export interface WatchedFolder {
  id: string;
  path: string;
  isActive: boolean;
  trackCount: number;
  addedAt: number;
}

/**
 * The filesystem walk. Separate from `AnalysisProgress` because the walk is
 * IO-bound and finishes long before the CPU stage does — one bar for both
 * would sit at 100% for an hour.
 */
export interface ScanProgress {
  total: number;
  done: number;
  skipped: number;
  failed: number;
  currentPath: string | null;
  isScanning: boolean;
}

export interface ReconcileReport {
  seen: number;
  touched: number;
  markedMissing: number;
  elapsedMs: number;
}

export interface CleanCacheReport {
  tracksRemoved: number;
  analysisRowsRemoved: number;
  peakFilesRemoved: number;
  bytesFreed: number;
}

/** A plain folder listing for the ADD TRACKS flow. Keystone has no pack tree:
    a DJ library is flat, and the wheel plus collections do the narrowing. */
export interface LibraryFolder {
  path: string;
  name: string;
  trackCount: number;
}

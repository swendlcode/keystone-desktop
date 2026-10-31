/** Cue-point export (SPEC.md §3 Export Cue Points, ENGINE.md §6). */
export type ExportTarget = 'rekordbox' | 'traktor' | 'serato';

/**
 * What detection found for one target. Mirrors `export::TargetStatus`.
 *
 * `installed: false` is why the SPEC's checkbox is disabled with an
 * explanation. The path is reported as two fields rather than one because the
 * CHANGE button is mandatory: Traktor makes a new root on every version update
 * and the root is user-settable in its own preferences, so an override has to
 * be distinguishable from what detection guessed.
 */
export interface TargetStatus {
  target: ExportTarget;
  installed: boolean;
  /** What detection found, which may be stale or wrong. */
  detectedPath: string | null;
  /** What the user chose with CHANGE. Wins over `detectedPath`. */
  overridePath: string | null;
  /** The sentence the disabled checkbox shows. */
  reasonUnavailable: string | null;
  /** True while the DJ app is open — a refusal, not a warning. */
  appRunning: boolean;
  /** v1.1. Serato is not shippable in v1. */
  supported: boolean;
  /** What the user must do in the DJ app afterwards. */
  manualSteps: string[];
}

/** The path a write would go to: the override if there is one. */
export function targetPath(status: TargetStatus): string | null {
  return status.overridePath ?? status.detectedPath;
}

/** One export run's arguments. Mirrors `export::ExportRequest`. */
export interface ExportRequest {
  target: ExportTarget;
  trackIds: string[];
  /** SPEC.md §3's `Overwrite cue points already in the collection`. */
  overwriteExisting: boolean;
  dryRun: boolean;
  /** Null lets the backend resolve it from the target's settings. */
  outputPath: string | null;
  /** Groups the run's log rows. The dry run allocates it and the write reuses
      it, so an interrupted run is reportable rather than half-applied. */
  runId: string;
}

/**
 * The plan *and* the result. Mirrors `export::ExportReport`.
 *
 * One type for both because the backend produces both from the same code path
 * with the write suppressed; two would let the plan drift from what the write
 * actually does, and a plan that disagrees with the write is worse than none.
 */
export interface ExportReport {
  runId: string;
  /** Null only on an empty report; a real run always names its target. */
  target: ExportTarget | null;
  /** Where the bytes went, or would have gone. */
  destination: string | null;
  planned: number;
  written: number;
  verified: number;
  skipped: number;
  failed: number;
  tracksMatched: number;
  tracksMissing: number;
  cuesToWrite: number;
  cuesToOverwrite: number;
  backupPath: string | null;
  manualSteps: string[];
  failures: ExportFailure[];
}

export interface ExportFailure {
  trackId: string;
  path: string;
  reason: string;
}

/** A dated copy taken before a write to a target. Mirrors `safety::BackupEntry`. */
export interface BackupEntry {
  path: string;
  target: ExportTarget;
  /** Seconds since the epoch, from the file's own mtime. */
  createdUnix: number;
  sizeBytes: number;
}

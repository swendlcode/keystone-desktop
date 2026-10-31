import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { KeystoneEvent, type FileProgressEvent } from '../../types';

interface RunOptions<P, R> {
  /** The dry run. Its result is what the confirmation shows. */
  dryRun: () => Promise<P>;
  /** The real write. Only ever called from `confirm`. */
  apply: (plan: P) => Promise<R>;
  /** Invalidate queries, refresh the table — whatever the write changed. */
  onDone?: (result: R) => void;
}

export interface FileWriteRun<P, R> {
  /** The dry-run result, awaiting confirmation. Null when nothing is pending. */
  plan: P | null;
  /** The applied result, for the undo banner. */
  result: R | null;
  /** How far the write has got, while one is running. Null when idle. */
  progress: FileProgressEvent | null;
  busy: boolean;
  error: string | null;
  /** Runs the dry run and opens the confirmation. */
  start: () => void;
  confirm: () => void;
  cancel: () => void;
  clearResult: () => void;
}

/**
 * plan -> confirm -> apply, for everything in Keystone that rewrites a file.
 *
 * The shape is the same for a bulk tag replace, a re-tag run, a rename and a
 * cue-point export, and it is one shape on purpose: the engine plan's safety
 * rule is that a write is always preceded by a dry run whose counts the user
 * sees. Spelling that out four times is four chances to forget it once.
 *
 * `plan` is held rather than discarded because `apply` is handed the plan, not
 * the arguments: the export path needs the `runId` the dry run allocated, so
 * re-deriving it at confirm time would write under a run nobody logged.
 */
export function useFileWriteRun<P, R>({
  dryRun,
  apply,
  onDone,
}: RunOptions<P, R>): FileWriteRun<P, R> {
  const [plan, setPlan] = useState<P | null>(null);
  const [result, setResult] = useState<R | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<FileProgressEvent | null>(null);

  // The backend reports every re-tag, rename and cue export on these two
  // events, and without a listener a 5,000-file write is a frozen dialog.
  //
  // Not filtered by `runId`: a tag run's id is allocated inside the command and
  // only comes back with the report, so there is nothing to filter against
  // until the write has already finished. These flows are modal and sequential
  // — one is in flight at a time — so the newest event is this run's.
  useEffect(() => {
    if (!busy) return;
    const unsubs = [
      listen<FileProgressEvent>(KeystoneEvent.tagProgress, (e) => setProgress(e.payload)),
      listen<FileProgressEvent>(KeystoneEvent.exportProgress, (e) => setProgress(e.payload)),
    ];
    return () => {
      Promise.all(unsubs).then((fns) => fns.forEach((fn) => fn()));
    };
  }, [busy]);

  const start = useCallback(() => {
    setError(null);
    setResult(null);
    setProgress(null);
    setBusy(true);
    dryRun()
      .then(setPlan)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setBusy(false));
  }, [dryRun]);

  const confirm = useCallback(() => {
    if (plan == null) return;
    setBusy(true);
    apply(plan)
      .then((r) => {
        setResult(r);
        setPlan(null);
        onDone?.(r);
      })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setBusy(false));
  }, [apply, onDone, plan]);

  const cancel = useCallback(() => {
    setPlan(null);
    setError(null);
  }, []);

  return {
    plan,
    result,
    progress,
    busy,
    error,
    start,
    confirm,
    cancel,
    clearResult: () => setResult(null),
  };
}

import { ConfirmDialog } from './ConfirmDialog';
import { RunProgress } from './RunProgress';
import { WriteReportBanner } from './WriteReportBanner';
import { useTagUndo } from './useTagUndo';
import type { FileWriteRun } from './useFileWriteRun';
import type { TagWriteReport } from '../../types';

/**
 * The confirmation and the result banner for a bulk tag write.
 *
 * All three bulk writes — re-tag, global replace, remove extra spaces — have
 * the same two moments: a dry-run count to confirm, and a result with Undo. One
 * component rather than three copies, because the thing being guaranteed here
 * is that no bulk write ever skips either moment, and three copies is three
 * chances to skip one.
 *
 * A zero count cannot be confirmed: the button says so and closes instead.
 */
export function TagRunDialogs({
  run,
  trackIds,
  title,
  verb,
}: {
  run: FileWriteRun<TagWriteReport, TagWriteReport>;
  trackIds: string[];
  title: string;
  /** The confirm button's verb — "Write", "Change", "Tidy". */
  verb: string;
}) {
  const undo = useTagUndo();

  return (
    <>
      {run.plan && (
        <ConfirmDialog
          title={title}
          destructive
          busy={run.busy}
          confirmLabel={
            run.plan.written > 0 ? `${verb} ${run.plan.written}` : 'Nothing to change'
          }
          body={
            <>
              <span className="mono">
                {run.plan.written} would change, {run.plan.skipped} already correct,{' '}
                {run.plan.failed} unreadable.
              </span>
              <RunProgress progress={run.progress} />
            </>
          }
          onCancel={run.cancel}
          onConfirm={run.plan.written > 0 ? run.confirm : run.cancel}
        />
      )}

      {run.result && (
        <WriteReportBanner
          report={run.result}
          // Undo restores the newest backup of each id it is handed, so with no
          // ids it has nothing to walk. A library-wide run is undone per track
          // from the track's own backup list, not from here, and offering a
          // button that would report "0 restored" is worse than not offering it.
          onUndo={trackIds.length > 0 ? () => undo.undo(trackIds) : undefined}
          undoBusy={undo.busy}
          undone={undo.restored}
          onDismiss={run.clearResult}
        />
      )}
    </>
  );
}

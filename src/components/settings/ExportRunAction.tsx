import { useCallback, useMemo } from 'react';
import { Button } from '../ui/Button';
import { ExportCurve, TickCircle } from '../ui/icons';
import { keystoneService } from '../../services/keystoneService';
import { useLibraryStore } from '../../stores/libraryStore';
import { ConfirmDialog } from './ConfirmDialog';
import { RunProgress } from './RunProgress';
import { SettingNote } from './SettingSection';
import { TARGET_LABEL } from './useExportTargets';
import { useFileWriteRun } from './useFileWriteRun';
import type { ExportReport, ExportRequest, ExportTarget } from '../../types';

/** A report paired with the target it came from. */
interface TargetRun {
  target: ExportTarget;
  request: ExportRequest;
  report: ExportReport;
}

/**
 * EXPORT CUE POINTS FOR EXISTING FILES, across every enabled target.
 *
 * One dry run per target before anything is written, and the dialog carries the
 * manual steps with the counts. That is not padding: Keystone cannot push
 * anything into rekordbox — the user has to import the XML and answer yes to a
 * prompt about tag information, and an export that silently "succeeded" without
 * those steps is the single most predictable support ticket this feature has.
 *
 * The target travels beside the report rather than being read out of it. The
 * backend's `target` is nullable on an empty report, and the thing this needs to
 * label a row is which target it *asked* about.
 */
export function ExportRunAction({
  targets,
  overwrite,
}: {
  targets: ExportTarget[];
  overwrite: boolean;
}) {
  const selectedIds = useLibraryStore((s) => s.selectedIds);
  const ids = useMemo(() => [...selectedIds], [selectedIds]);

  const dryRun = useCallback(
    () =>
      Promise.all(
        targets.map(async (target) => {
          // One run id per target, allocated here and reused by the write, so
          // the log rows of an interrupted run can be found and finished.
          const request: ExportRequest = {
            target,
            trackIds: ids,
            overwriteExisting: overwrite,
            dryRun: true,
            outputPath: null,
            runId: crypto.randomUUID(),
          };
          return {
            target,
            request,
            report: await keystoneService.planCueExport(request),
          };
        }),
      ),
    [ids, overwrite, targets],
  );

  const apply = useCallback(async (plans: TargetRun[]) => {
    const results: TargetRun[] = [];
    // Sequential on purpose: each target backs up and writes a library file,
    // and two of those running at once is how a half-written collection gets
    // read back in.
    for (const plan of plans) {
      const request = { ...plan.request, dryRun: false };
      results.push({
        target: plan.target,
        request,
        report: await keystoneService.exportCuePoints(request),
      });
    }
    return results;
  }, []);

  const run = useFileWriteRun<TargetRun[], TargetRun[]>({ dryRun, apply });
  const total = run.plan?.reduce((n, p) => n + p.report.cuesToWrite, 0) ?? 0;

  return (
    <div className="flex flex-col gap-2">
      <Button
        variant="primary"
        size="sm"
        className="self-start"
        disabled={run.busy || targets.length === 0 || ids.length === 0}
        onClick={run.start}
        icon={<ExportCurve size={13} color="currentColor" variant="Linear" />}
      >
        EXPORT CUE POINTS FOR EXISTING FILES
      </Button>

      <SettingNote>{scopeNote(targets.length, ids.length)}</SettingNote>

      {run.error && <SettingNote tone="warn">{run.error}</SettingNote>}

      {run.plan && (
        <ConfirmDialog
          title="Export cue points?"
          busy={run.busy}
          confirmLabel={total > 0 ? `Write ${total} cues` : 'Nothing to write'}
          body={
            <>
              <PlanBody plans={run.plan} />
              <RunProgress progress={run.progress} />
            </>
          }
          onCancel={run.cancel}
          onConfirm={total > 0 ? run.confirm : run.cancel}
        />
      )}

      {run.result && <ResultBanner results={run.result} onDismiss={run.clearResult} />}
    </div>
  );
}

/** Why the button is disabled, or what pressing it will touch. */
function scopeNote(targets: number, selected: number): string {
  if (targets === 0) return 'Pick a target above.';
  if (selected === 0) return 'Select tracks in the library first.';
  return `${selected} selected. A backup is taken before the first write.`;
}

function ResultBanner({
  results,
  onDismiss,
}: {
  results: TargetRun[];
  onDismiss: () => void;
}) {
  return (
    <div className="flex items-start gap-2 rounded-lg border border-gray-700 bg-gray-800 p-3">
      <span className="mt-0.5 text-keystone-lime">
        <TickCircle size={14} color="currentColor" variant="Linear" />
      </span>
      <div className="min-w-0 flex-1">
        {results.map(({ target, report }) => (
          <p key={target} className="mono text-[11px] text-keystone-white">
            {TARGET_LABEL[target]}: {report.written} written, {report.failed} failed
          </p>
        ))}
      </div>
      <Button variant="ghost" size="sm" onClick={onDismiss}>
        Dismiss
      </Button>
    </div>
  );
}

function PlanBody({ plans }: { plans: TargetRun[] }) {
  return (
    <div className="flex flex-col gap-3">
      {plans.map(({ target, report }) => (
        <div key={target}>
          <p className="mono text-[11px] text-keystone-white">
            {TARGET_LABEL[target]} — {report.cuesToWrite} cues,{' '}
            {report.cuesToOverwrite} replaced, {report.tracksMissing} tracks not in
            its library
          </p>
          {report.manualSteps.length > 0 && (
            <ol className="mt-1 list-inside list-decimal text-[11px] text-gray-400">
              {report.manualSteps.map((s) => (
                <li key={s}>{s}</li>
              ))}
            </ol>
          )}
        </div>
      ))}
    </div>
  );
}

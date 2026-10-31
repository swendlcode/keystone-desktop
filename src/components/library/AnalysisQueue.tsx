import { Cpu, Pause, Play, Trash } from '../ui/icons';
import { ProgressBar } from '../ui/ProgressBar';
import { Tooltip } from '../ui/Tooltip';
import {
  queueFraction,
  queueIsEmpty,
  useAnalysisStore,
} from '../../stores/analysisStore';
import { useAnalysisProgress, useQueueControls } from '../../hooks/useAnalysis';
import { formatCount } from '../../utils/formatters';

/**
 * The sidebar's Analysis Queue row.
 *
 * `Empty` is the idle rendering, as SPEC.md writes it — not `0%`, which reads
 * as stalled. When the queue has work but no measured rate the bar is
 * indeterminate rather than showing an invented estimate.
 */
export function AnalysisQueue() {
  // Through the hook rather than straight off the store: the store starts empty
  // and is only written by the event stream, so a window opened while work is
  // already queued would read `Empty` until the next tick. The hook seeds it
  // from `get_analysis_progress` once and then steps aside.
  const progress = useAnalysisProgress();
  const lastRun = useAnalysisStore((s) => s.lastRun);
  const { pause, resume, clear } = useQueueControls();

  const empty = queueIsEmpty(progress);
  const fraction = queueFraction(progress);
  const running = progress?.state === 'running';

  return (
    <div className="border-y border-gray-700 px-3 py-2.5">
      <div className="flex items-center justify-between">
        <span className="label flex items-center gap-1.5">
          <Cpu size={12} color="currentColor" variant="Linear" />
          Analysis Queue
        </span>
        {!empty && (
          <span className="flex items-center gap-0.5">
            <Tooltip label={running ? 'Pause' : 'Resume'}>
              <button
                type="button"
                aria-label={running ? 'Pause queue' : 'Resume queue'}
                onClick={() => (running ? pause.mutate() : resume.mutate())}
                className="flex h-6 w-6 items-center justify-center rounded text-gray-400 hover:bg-gray-800 hover:text-keystone-white"
              >
                {running ? (
                  <Pause size={12} color="currentColor" variant="Bold" />
                ) : (
                  <Play size={12} color="currentColor" variant="Bold" />
                )}
              </button>
            </Tooltip>
            <Tooltip label="Clear queue">
              <button
                type="button"
                aria-label="Clear queue"
                onClick={() => clear.mutate()}
                className="flex h-6 w-6 items-center justify-center rounded text-gray-400 hover:bg-gray-800 hover:text-keystone-white"
              >
                <Trash size={12} color="currentColor" variant="Linear" />
              </button>
            </Tooltip>
          </span>
        )}
      </div>

      {empty ? (
        <p className="mono mt-1 text-[11px] text-gray-500">
          {lastRun
            ? `Done — ${formatCount(lastRun.analyzed)} analysed, ${formatCount(lastRun.fromCache)} cached`
            : 'Empty'}
        </p>
      ) : (
        <div className="mt-1.5 flex flex-col gap-1.5">
          <ProgressBar value={fraction} />
          <div className="mono flex items-center justify-between text-[11px] text-gray-400">
            <span className="tnum">
              {formatCount(progress?.done ?? 0)} / {formatCount(progress?.total ?? 0)}
            </span>
            {(progress?.failed ?? 0) > 0 && (
              <span className="text-amber-400">{formatCount(progress!.failed)} failed</span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

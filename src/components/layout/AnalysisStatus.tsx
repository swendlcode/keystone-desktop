import { useEffect, useState } from 'react';
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

/** How long the finished summary stays up before the strip clears itself. */
const DONE_VISIBLE_MS = 8000;

/**
 * The analysis queue, in the status bar, only while there is something to say.
 *
 * It used to be a permanent block in the sidebar, under the wheel, reading
 * `Empty` for the whole of an ordinary session. That is a titled section, two
 * dividers and two lines of chrome spent on the answer "nothing is happening",
 * paid for out of the column that holds the wheel and the collections, which
 * is what the sidebar is for. Progress is a transient, and a transient belongs
 * on the status bar: it appears when work starts, says how far along it is,
 * and gives the space back when it is done.
 *
 * The finished summary lingers a few seconds rather than vanishing on the last
 * track, because "did that actually run?" is worth answering for someone who
 * was watching the table and not this strip.
 */
export function AnalysisStatus() {
  // Through the hook rather than straight off the store: the store starts
  // empty and is only written by the event stream, so a window opened while
  // work is already queued would show nothing until the next tick. The hook
  // seeds it from `get_analysis_progress` once and then steps aside.
  const progress = useAnalysisProgress();
  const lastRun = useAnalysisStore((s) => s.lastRun);
  const { pause, resume, clear } = useQueueControls();

  const empty = queueIsEmpty(progress);
  const [showDone, setShowDone] = useState(false);

  useEffect(() => {
    if (!lastRun) return;
    setShowDone(true);
    const t = setTimeout(() => setShowDone(false), DONE_VISIBLE_MS);
    return () => clearTimeout(t);
  }, [lastRun]);

  if (empty) {
    if (!showDone || !lastRun) return null;
    return (
      <span className="mono flex items-center gap-1.5 text-[11px] text-gray-500">
        <Cpu size={12} color="currentColor" variant="Linear" />
        Analysed {formatCount(lastRun.analyzed)}
        {lastRun.fromCache > 0 && ` · ${formatCount(lastRun.fromCache)} cached`}
        {lastRun.failed > 0 && (
          <span className="text-amber-400">
            {' '}
            · {formatCount(lastRun.failed)} failed
          </span>
        )}
      </span>
    );
  }

  const running = progress?.state === 'running';

  return (
    <span className="flex items-center gap-2">
      <span className="label flex items-center gap-1.5">
        <Cpu size={12} color="currentColor" variant="Linear" />
        Analysing
      </span>

      {/* A fixed width, because a bar sized by its own label is a bar that
          twitches on every completed track. */}
      <ProgressBar value={queueFraction(progress)} className="w-28" />

      <span className="mono tnum text-[11px] text-gray-400">
        {formatCount(progress?.done ?? 0)} / {formatCount(progress?.total ?? 0)}
      </span>
      {(progress?.failed ?? 0) > 0 && (
        <span className="mono tnum text-[11px] text-amber-400">
          {formatCount(progress!.failed)} failed
        </span>
      )}

      <Tooltip label={running ? 'Pause' : 'Resume'}>
        <button
          type="button"
          aria-label={running ? 'Pause queue' : 'Resume queue'}
          onClick={() => (running ? pause.mutate() : resume.mutate())}
          className="flex h-5 w-5 items-center justify-center rounded text-gray-400 hover:bg-gray-800 hover:text-keystone-white"
        >
          {running ? (
            <Pause size={11} color="currentColor" variant="Bold" />
          ) : (
            <Play size={11} color="currentColor" variant="Bold" />
          )}
        </button>
      </Tooltip>
      <Tooltip label="Clear queue">
        <button
          type="button"
          aria-label="Clear queue"
          onClick={() => clear.mutate()}
          className="flex h-5 w-5 items-center justify-center rounded text-gray-400 hover:bg-gray-800 hover:text-keystone-white"
        >
          <Trash size={11} color="currentColor" variant="Linear" />
        </button>
      </Tooltip>
    </span>
  );
}

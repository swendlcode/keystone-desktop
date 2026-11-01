import { Button } from '../ui/Button';
import { TickCircle, Warning2 } from '../ui/icons';
import type { TagWriteReport } from '../../types';

/**
 * What a finished tag run did, with Undo beside it.
 *
 * The undo is on the result, not buried in a menu: it rewrote the user's files,
 * and the only moment they can judge whether that was right is immediately
 * after, with the count in front of them.
 */
export function WriteReportBanner({
  report,
  onUndo,
  undoBusy,
  undone,
  onDismiss,
}: {
  report: TagWriteReport;
  onUndo?: () => void;
  undoBusy?: boolean;
  undone?: number | null;
  onDismiss: () => void;
}) {
  const bad = report.failed > 0;

  return (
    <div
      className={`flex items-start gap-3 rounded-lg border p-3 ${
        bad ? 'border-amber-500/40 bg-amber-500/10' : 'border-gray-700 bg-gray-800'
      }`}
    >
      <span className={bad ? 'mt-0.5 text-amber-400' : 'mt-0.5 text-keystone-lime'}>
        {bad ? (
          <Warning2 size={14} color="currentColor" variant="Linear" />
        ) : (
          <TickCircle size={14} color="currentColor" variant="Linear" />
        )}
      </span>

      <div className="min-w-0 flex-1">
        <p className="mono text-[12px] text-keystone-white">
          {undone != null
            ? `${undone} restored`
            : `${report.written} written, ${report.skipped} unchanged, ${report.failed} failed`}
        </p>
        {report.failures.slice(0, 3).map((f) => (
          <p key={f.trackId} className="mono mt-0.5 truncate text-[12px] text-amber-400">
            {f.path}: {f.reason}
          </p>
        ))}
      </div>

      {onUndo && undone == null && (
        <Button variant="ghost" size="sm" disabled={undoBusy} onClick={onUndo}>
          {undoBusy ? 'Undoing...' : 'Undo'}
        </Button>
      )}
      <Button variant="ghost" size="sm" onClick={onDismiss}>
        Dismiss
      </Button>
    </div>
  );
}

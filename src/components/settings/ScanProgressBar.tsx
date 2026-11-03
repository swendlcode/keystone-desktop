import { Button } from '../ui/Button';
import { ProgressBar } from '../ui/ProgressBar';
import { useLibraryStore } from '../../stores/libraryStore';

/**
 * How far the filesystem walk has got, while it is running.
 *
 * Renders nothing when nothing is scanning: an empty bar on a quiet library
 * reads as something stuck. `skipped` is shown as "unchanged" rather than as a
 * failure count — the walk skips a path whose mtime has not moved, so on a
 * re-scan of a large library almost everything is skipped and that is the
 * feature working, not a problem.
 */
export function ScanProgressBar({
  onCancel,
  cancelling,
}: {
  onCancel: () => void;
  cancelling: boolean;
}) {
  const progress = useLibraryStore((s) => s.scanProgress);
  if (!progress?.isScanning) return null;

  const { done, total, skipped, failed, currentPath } = progress;
  return (
    <div className="mt-2 flex flex-col gap-1">
      <ProgressBar value={total > 0 ? done / total : 0} />
      <div className="flex items-center justify-between gap-3">
        <p className="mono truncate text-[12px] text-gray-500">
          {done} of {total}
          {skipped > 0 && ` \u00b7 ${skipped} unchanged`}
          {failed > 0 && ` \u00b7 ${failed} failed`}
        </p>
        <Button variant="ghost" size="sm" disabled={cancelling} onClick={onCancel}>
          Cancel
        </Button>
      </div>
      {currentPath && (
        <p className="mono truncate text-[11px] text-gray-600">{currentPath}</p>
      )}
    </div>
  );
}

import { Button } from '../ui/Button';
import { Trash } from '../ui/icons';
import { formatCount } from '../../utils/formatters';
import type { CleanCacheReport, ReconcileReport, WatchedFolder } from '../../types';

/**
 * One watched folder, and the two result lines the maintenance actions print.
 *
 * Split out of `LibraryPane` only for the 200-line cap; they have no use
 * anywhere else.
 */
export function FolderRow({
  folder,
  busy,
  onRescan,
  onRemove,
}: {
  folder: WatchedFolder;
  busy: boolean;
  onRescan: () => void;
  onRemove: () => void;
}) {
  return (
    <div className="group/row flex items-center gap-3 rounded-md border border-gray-700/50 px-3 py-2">
      <span className="min-w-0 flex-1">
        <span className="block truncate text-xs text-keystone-white">
          {folder.path.split('/').pop() || folder.path}
        </span>
        {/* The full path under the name: two folders called `Tracks` under
            different drives are the normal case in a DJ library. */}
        <span className="mono mt-0.5 block truncate text-[11px] text-gray-500">
          {folder.path}
        </span>
      </span>
      <span className="mono tnum shrink-0 rounded bg-gray-700 px-1.5 py-0.5 text-[11px] leading-none text-gray-300">
        {formatCount(folder.trackCount)}
      </span>
      <Button
        variant="ghost"
        size="sm"
        disabled={busy}
        title="Look for files added or changed outside Keystone"
        onClick={onRescan}
      >
        Re-scan
      </Button>
      <button
        type="button"
        aria-label={`Stop watching ${folder.path}`}
        title="Stop watching"
        onClick={onRemove}
        className="rounded p-1 text-gray-500 transition-colors hover:bg-red-500/10 hover:text-red-400"
      >
        <Trash size={13} color="currentColor" variant="Linear" />
      </button>
    </div>
  );
}

export function ReconcileResult({ report }: { report: ReconcileReport }) {
  return (
    <p className="mono text-[12px] text-gray-400">
      Checked {formatCount(report.seen)} tracks in {Math.round(report.elapsedMs)}ms
      {report.markedMissing > 0
        ? ` \u00b7 ${formatCount(report.markedMissing)} now missing`
        : ' \u00b7 nothing missing'}
    </p>
  );
}

export function CleanResult({ report }: { report: CleanCacheReport }) {
  return (
    <p className="mono text-[12px] text-gray-400">
      Removed {formatCount(report.tracksRemoved)} tracks,{' '}
      {formatCount(report.analysisRowsRemoved)} analyses,{' '}
      {formatCount(report.peakFilesRemoved)} waveform files \u00b7{' '}
      {formatBytes(report.bytesFreed)} freed
    </p>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

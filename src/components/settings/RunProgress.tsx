import { ProgressBar } from '../ui/ProgressBar';
import type { FileProgressEvent } from '../../types';

/**
 * How far a bulk write has got, from `keystone://tag-progress` or
 * `keystone://export-progress`.
 *
 * Renders nothing until the first event arrives. The backend emits every
 * hundredth file rather than every file, so a run that is genuinely working can
 * sit on one number for a moment — which is still worth showing, because the
 * alternative over several thousand files is a dialog that looks hung.
 */
export function RunProgress({ progress }: { progress: FileProgressEvent | null }) {
  if (!progress || progress.total === 0) return null;
  const { done, total, failed } = progress;
  return (
    <div className="mt-2 flex flex-col gap-1">
      <ProgressBar value={done / total} />
      <p className="mono text-[11px] text-gray-500">
        {done} of {total}
        {failed > 0 && `, ${failed} failed`}
      </p>
    </div>
  );
}

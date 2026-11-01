import { useFileDrop } from '../../hooks/useFileDrop';
import { DocumentUpload } from '../ui/icons';

/**
 * The whole window is the drop target.
 *
 * Confining the drop to a panel means learning where the panel is before the
 * app is useful, and the gesture people actually make is "throw this at the
 * app". The overlay only appears while something is over the window, so it
 * costs nothing the rest of the time.
 *
 * `pointer-events-none` matters: a visible layer that swallowed the drag would
 * make the drop fail the moment the user saw the invitation to make it.
 */
export function DropOverlay() {
  const hovering = useFileDrop();
  if (!hovering) return null;

  return (
    <div className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-keystone-black/70 backdrop-blur-sm">
      <div className="flex flex-col items-center gap-3 rounded-2xl border-2 border-dashed border-keystone-lime/60 bg-gray-900/80 px-14 py-10">
        <DocumentUpload size={30} color="currentColor" variant="Linear" className="text-keystone-lime" />
        <span className="text-sm font-semibold text-keystone-white">Drop to analyse</span>
        <span className="text-xs text-gray-400">Tracks or folders</span>
      </div>
    </div>
  );
}

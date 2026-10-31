import type { ReactNode } from 'react';
import {
  Next,
  Pause,
  Play,
  Previous,
  SearchZoomIn,
  SearchZoomOut,
} from '../ui/icons';
import { MAX_ZOOM, usePlayerStore } from '../../stores/playerStore';
import { formatTime } from '../../utils/formatters';

/**
 * Transport, zoom and the elapsed / total readout.
 *
 * The play button calls `pause()` / `resume()` and never `stop()`. They land
 * on the same `isPlaying: false`, but stop rewinds, and a row's play button
 * that rewinds is how the spacebar stopped resuming mid-track in the sibling
 * app — see the note on `stopSeq` in `playerStore`.
 */
export function PlaybackControls({ totalS }: { totalS: number }) {
  const isPlaying = usePlayerStore((s) => s.isPlaying);
  const hasTrack = usePlayerStore((s) => s.currentTrack != null);
  const currentTime = usePlayerStore((s) => s.currentTime);
  const zoom = usePlayerStore((s) => s.zoom);
  const pause = usePlayerStore((s) => s.pause);
  const resume = usePlayerStore((s) => s.resume);
  const playNext = usePlayerStore((s) => s.playNext);
  const playPrev = usePlayerStore((s) => s.playPrev);
  const zoomIn = usePlayerStore((s) => s.zoomIn);
  const zoomOut = usePlayerStore((s) => s.zoomOut);

  return (
    <div className="flex items-center gap-2">
      <IconButton label="Previous" onClick={playPrev} disabled={!hasTrack}>
        <Previous size={15} color="currentColor" variant="Bold" />
      </IconButton>

      <button
        type="button"
        aria-label={isPlaying ? 'Pause' : 'Play'}
        disabled={!hasTrack}
        onClick={() => (isPlaying ? pause() : resume())}
        className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-keystone-lime text-[color:var(--color-on-accent)] transition-colors hover:bg-[color:var(--color-accent-hover)] disabled:opacity-30"
      >
        {isPlaying ? (
          <Pause size={16} color="currentColor" variant="Bold" />
        ) : (
          <Play size={16} color="currentColor" variant="Bold" />
        )}
      </button>

      <IconButton label="Next" onClick={playNext} disabled={!hasTrack}>
        <Next size={15} color="currentColor" variant="Bold" />
      </IconButton>

      <span className="mono tnum ml-1 shrink-0 text-xs text-gray-400">
        {formatTime(currentTime)}
        <span className="text-gray-600"> / </span>
        {formatTime(totalS)}
      </span>

      <span className="ml-auto flex items-center gap-0.5">
        <IconButton label="Zoom out" onClick={zoomOut} disabled={zoom === 0}>
          <SearchZoomOut size={14} color="currentColor" variant="Linear" />
        </IconButton>
        <span className="mono tnum w-6 text-center text-[11px] text-gray-500">
          {2 ** zoom}x
        </span>
        <IconButton
          label="Zoom in"
          onClick={zoomIn}
          disabled={zoom === MAX_ZOOM}
        >
          <SearchZoomIn size={14} color="currentColor" variant="Linear" />
        </IconButton>
      </span>
    </div>
  );
}

function IconButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      disabled={disabled}
      className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-gray-400 transition-colors hover:bg-gray-800 hover:text-keystone-white disabled:opacity-30 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}

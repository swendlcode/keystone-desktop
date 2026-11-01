import type { ReactNode } from 'react';
import {
  Next,
  Play,
  Previous,
  SearchZoomIn,
  SearchZoomOut,
  Stop,
} from '../ui/icons';
import { VolumeControl } from './VolumeControl';
import { MAX_ZOOM, usePlayerStore } from '../../stores/playerStore';
import { formatTime } from '../../utils/formatters';

/**
 * Transport: previous, play/pause, next — beside the waveform, not above it.
 *
 * The play button is a filled accent disc with an OUTLINE glyph on it. Every
 * icon in Keystone is `variant="Linear"`; a Bold glyph here was a second
 * icon style in an app that has one, and the solid triangle on a solid disc
 * read as a single blob rather than as a mark on a button. The disc carries
 * the emphasis, the glyph carries the meaning.
 *
 * It calls `pause()` / `resume()` and never `stop()`. They land on the same
 * `isPlaying: false`, but stop rewinds, and a play button that rewinds is how
 * the spacebar stopped resuming mid-track in the sibling app — see the note
 * on `stopSeq` in `playerStore`.
 */
export function PlaybackControls() {
  const isPlaying = usePlayerStore((s) => s.isPlaying);
  const hasTrack = usePlayerStore((s) => s.currentTrack != null);
  const pause = usePlayerStore((s) => s.pause);
  const resume = usePlayerStore((s) => s.resume);
  const playNext = usePlayerStore((s) => s.playNext);
  const playPrev = usePlayerStore((s) => s.playPrev);

  return (
    <div className="flex shrink-0 items-center gap-1.5">
      <IconButton label="Previous" onClick={playPrev} disabled={!hasTrack}>
        <Previous size={14} color="currentColor" variant="Linear" />
      </IconButton>

      <button
        type="button"
        aria-label={isPlaying ? 'Pause' : 'Play'}
        disabled={!hasTrack}
        onClick={() => (isPlaying ? pause() : resume())}
        className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-keystone-lime text-[color:var(--color-on-accent)] shadow-lg shadow-keystone-lime/20 transition-colors hover:bg-[color:var(--color-accent-hover)] disabled:bg-gray-700 disabled:text-gray-500 disabled:shadow-none"
      >
        {isPlaying ? (
          // A square, not the two-bar pause glyph. The button still pauses —
          // it calls `pause()`, holds the playhead and resumes from there —
          // so the square is the shape asked for, not a change of behaviour.
          // Worth knowing if the icon ever tempts someone to wire it to
          // `stop()`, which rewinds, and is the bug the note in `playerStore`
          // is about.
          <Stop size={17} color="currentColor" variant="Linear" />
        ) : (
          // Nudged right by a pixel: a triangle's visual centre is left of
          // its bounding box, so centring the box leaves it looking off.
          <Play size={18} color="currentColor" variant="Linear" className="ml-0.5" />
        )}
      </button>

      <IconButton label="Next" onClick={playNext} disabled={!hasTrack}>
        <Next size={14} color="currentColor" variant="Linear" />
      </IconButton>
    </div>
  );
}

/**
 * Elapsed, total and zoom — overlaid in the waveform's bottom-right corner.
 *
 * They belong to the waveform rather than to the transport, and giving them
 * their own row above it cost a whole line of the player bar to six
 * characters and two buttons.
 */
export function WaveformMeta({ totalS }: { totalS: number }) {
  const currentTime = usePlayerStore((s) => s.currentTime);
  const zoom = usePlayerStore((s) => s.zoom);
  const zoomIn = usePlayerStore((s) => s.zoomIn);
  const zoomOut = usePlayerStore((s) => s.zoomOut);

  return (
    <div className="pointer-events-none absolute bottom-1 right-1 z-30 flex items-center gap-1">
      <span className="mono tnum rounded bg-gray-900/80 px-1.5 py-0.5 text-[11px] text-gray-300">
        {formatTime(currentTime)}
        <span className="text-gray-500"> / </span>
        {formatTime(totalS)}
      </span>
      <span className="pointer-events-auto flex items-center gap-0.5 rounded bg-gray-900/80 px-0.5">
        <MetaButton label="Zoom out" onClick={zoomOut} disabled={zoom === 0}>
          <SearchZoomOut size={13} color="currentColor" variant="Linear" />
        </MetaButton>
        <span className="mono tnum w-9 text-center text-[11px] text-gray-400">
          {2 ** zoom}x
        </span>
        <MetaButton
          label="Zoom in"
          onClick={zoomIn}
          disabled={zoom === MAX_ZOOM}
        >
          <SearchZoomIn size={13} color="currentColor" variant="Linear" />
        </MetaButton>
        <span className="mx-0.5 h-3.5 w-px bg-gray-700" aria-hidden />
        <VolumeControl />
      </span>
    </div>
  );
}

function MetaButton({
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
      onPointerDown={(e) => e.stopPropagation()}
      onClick={onClick}
      disabled={disabled}
      className="flex h-6 w-6 items-center justify-center rounded text-gray-400 transition-colors hover:text-keystone-white disabled:opacity-30"
    >
      {children}
    </button>
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
      className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-gray-700 text-gray-300 transition-colors hover:bg-gray-800 hover:text-keystone-white disabled:opacity-30 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}

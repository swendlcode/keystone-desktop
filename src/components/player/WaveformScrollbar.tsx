import { useRef, type PointerEvent as ReactPointerEvent } from 'react';
import { usePlayerStore } from '../../stores/playerStore';
import type { WaveWindow } from './waveformDraw';

/**
 * Where the visible window sits in the whole track, and a handle to move it.
 *
 * Zoomed in, the waveform shows a few seconds of a six-minute file with
 * nothing on screen saying which few — scrolling sideways was the only way
 * to find out, and it told you where you had arrived rather than where you
 * were. The thumb is the window and the track is the song, so the answer is
 * readable without moving anything.
 *
 * Overlaid on the waveform's bottom edge rather than placed under it. A strip
 * that appears below would add a row the moment you zoom and take it back the
 * moment you zoom out, which is the layout jumping on the one gesture most
 * likely to be repeated.
 */
export function WaveformScrollbar({
  window: win,
  durationS,
}: {
  window: WaveWindow;
  durationS: number;
}) {
  const zoom = usePlayerStore((s) => s.zoom);
  const dragging = useRef<{ grabOffset: number } | null>(null);

  // Zoom 0 is the whole track: the thumb would be the entire bar and say
  // nothing.
  if (zoom === 0 || !(durationS > 0)) return null;

  const span = Math.max(win.endS - win.startS, 1e-6);
  const left = win.startS / durationS;
  const width = Math.min(1, span / durationS);

  const centreFromClientX = (clientX: number, el: HTMLElement, grab: number) => {
    const rect = el.getBoundingClientRect();
    if (rect.width === 0) return null;
    // `grab` keeps the point of the thumb that was picked up under the
    // pointer, so the window does not jump on the first pixel of a drag.
    const frac = (clientX - rect.left) / rect.width - grab;
    return (frac + width / 2) * durationS;
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    if (rect.width === 0) return;
    const at = (e.clientX - rect.left) / rect.width;
    const inThumb = at >= left && at <= left + width;

    e.currentTarget.setPointerCapture(e.pointerId);
    dragging.current = { grabOffset: inThumb ? at - left : width / 2 };

    // A press on the empty part of the track jumps the window there, the
    // way clicking a scrollbar gutter does.
    const centre = centreFromClientX(e.clientX, e.currentTarget, dragging.current.grabOffset);
    if (centre != null) usePlayerStore.getState().setViewCentre(centre);
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragging.current;
    if (!drag) return;
    const centre = centreFromClientX(e.clientX, e.currentTarget, drag.grabOffset);
    if (centre != null) usePlayerStore.getState().setViewCentre(centre);
  };

  const end = (e: ReactPointerEvent<HTMLDivElement>) => {
    dragging.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };

  return (
    <div
      role="scrollbar"
      aria-label="Position in the track"
      aria-orientation="horizontal"
      aria-valuenow={Math.round(left * 100)}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={end}
      onPointerCancel={end}
      className="absolute inset-x-0 bottom-0 z-30 h-2 cursor-grab touch-none bg-gray-900/70 active:cursor-grabbing"
    >
      <span
        style={{ left: `${left * 100}%`, width: `${width * 100}%` }}
        // A floor on the width: at 128x on a long track the thumb is a
        // fraction of a pixel, and a handle you cannot see is not a handle.
        className="absolute inset-y-0.5 min-w-[18px] rounded-full bg-gray-500 transition-colors hover:bg-gray-400"
      />
    </div>
  );
}

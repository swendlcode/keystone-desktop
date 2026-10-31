import { useRef, useState, type PointerEvent, type RefObject } from 'react';
import { useMoveCuePoint } from '../../hooks/useCues';
import { usePlayerStore } from '../../stores/playerStore';
import { CUE_KIND_LABELS, type CuePoint } from '../../types';
import { formatTime } from '../../utils/formatters';
import type { WaveWindow } from './waveformDraw';

interface CueMarkersProps {
  cues: CuePoint[];
  window: WaveWindow;
  /** The element the markers are positioned inside. Measured on each drag. */
  containerRef: RefObject<HTMLDivElement>;
  onSeek: (seconds: number) => void;
}

/** Below this, a pointerup is a click to jump rather than the end of a drag. */
const DRAG_SLOP_PX = 3;

/**
 * CUE 1..8, drawn in place on the waveform.
 *
 * DOM elements over the canvas rather than shapes inside it. Eight of them is
 * not enough to cost anything, and it buys real hit targets, real focus and a
 * drag that does not need its own hit-testing — which is the half of this
 * feature that is actually fiddly.
 *
 * The move is committed on pointerup, never on pointermove: a drag across a
 * five-minute track is a hundred-odd move events and each one would be a
 * write to the user's database.
 */
export function CueMarkers({
  cues,
  window: win,
  containerRef,
  onSeek,
}: CueMarkersProps) {
  const move = useMoveCuePoint();
  const setDragging = usePlayerStore((s) => s.setDraggingCueSlot);
  const [preview, setPreview] = useState<{ slot: number; ms: number } | null>(
    null,
  );
  const drag = useRef<{ id: string; slot: number; startX: number; moved: boolean } | null>(
    null,
  );

  const spanS = Math.max(win.endS - win.startS, 1e-6);
  const msAtClientX = (clientX: number): number | null => {
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return null;
    const frac = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    return (win.startS + frac * spanS) * 1000;
  };

  const onPointerDown = (e: PointerEvent<HTMLButtonElement>, cue: CuePoint) => {
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { id: cue.id, slot: cue.slot, startX: e.clientX, moved: false };
    setDragging(cue.slot);
  };

  const onPointerMove = (e: PointerEvent<HTMLButtonElement>) => {
    const d = drag.current;
    if (!d) return;
    if (!d.moved && Math.abs(e.clientX - d.startX) < DRAG_SLOP_PX) return;
    d.moved = true;
    const ms = msAtClientX(e.clientX);
    if (ms != null) setPreview({ slot: d.slot, ms });
  };

  const onPointerUp = (e: PointerEvent<HTMLButtonElement>, cue: CuePoint) => {
    const d = drag.current;
    drag.current = null;
    setDragging(null);
    setPreview(null);
    if (!d) return;

    if (!d.moved) {
      onSeek(cue.positionMs / 1000);
      return;
    }
    const ms = msAtClientX(e.clientX);
    if (ms != null) move.mutate({ id: cue.id, positionMs: ms });
  };

  return (
    <>
      {cues.map((cue) => {
        const ms = preview?.slot === cue.slot ? preview.ms : cue.positionMs;
        const frac = (ms / 1000 - win.startS) / spanS;
        if (frac < -0.02 || frac > 1.02) return null;
        const color = cue.color ?? 'var(--color-accent)';

        return (
          <button
            key={cue.id}
            type="button"
            aria-label={`Cue ${cue.slot + 1}, ${formatTime(ms / 1000)}`}
            title={`CUE ${cue.slot + 1} — ${CUE_KIND_LABELS[cue.kind]} — ${formatTime(ms / 1000)}`}
            style={{ left: `${frac * 100}%`, color }}
            onPointerDown={(e) => onPointerDown(e, cue)}
            onPointerMove={onPointerMove}
            onPointerUp={(e) => onPointerUp(e, cue)}
            onPointerCancel={() => {
              drag.current = null;
              setDragging(null);
              setPreview(null);
            }}
            // -1px so the 2px line straddles the exact position rather than
            // sitting a pixel late, which is visible against the beat grid.
            className="absolute top-0 z-10 h-full w-3 -translate-x-1/2 cursor-ew-resize touch-none bg-transparent"
          >
            <span className="pointer-events-none absolute inset-y-0 left-1/2 w-0.5 -translate-x-1/2 bg-current opacity-80" />
            <span className="mono pointer-events-none absolute left-1/2 top-0 flex h-3.5 w-3.5 -translate-x-1/2 items-center justify-center rounded-sm bg-current text-[9px] leading-none text-[color:var(--color-on-accent)]">
              {cue.slot + 1}
            </span>
          </button>
        );
      })}
    </>
  );
}

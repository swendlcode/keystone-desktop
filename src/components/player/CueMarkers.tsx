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
  onContextMenu: (e: React.MouseEvent, cue: CuePoint) => void;
}

/** Below this, a pointerup is a click to jump rather than the end of a drag. */
const DRAG_SLOP_PX = 3;

/**
 * CUE 1..8, drawn on the waveform as flags.
 *
 * A flag rather than a bare tick: the label hangs off the top, to the side of
 * the line, so the line itself stays one hairline wide and keeps marking the
 * exact sample while the name is still readable. A numbered box centred on the
 * line — which is what this was — hid the position it was pointing at behind
 * its own badge.
 *
 * The flag is neutral with white text and the line carries the cue's colour.
 * Colouring the flag too made eight saturated blocks across the top of the
 * waveform, which competes with the waveform for the same glance.
 *
 * **The flag is part of the drag target.** It overflows the button's 12px box
 * and used to be `pointer-events-none`, so clicks on the label fell through to
 * the canvas and seeked instead of dragging — which made a cue look unmovable
 * unless you found the hairline. Everything drawn here is the button.
 *
 * The move is committed on pointerup, never on pointermove: a drag across a
 * five-minute track is a hundred-odd move events and each one would be a write
 * to the user's database.
 */
export function CueMarkers({
  cues,
  window: win,
  containerRef,
  onSeek,
  onContextMenu,
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
    // Right-click opens the menu; starting a drag on it would also move the
    // cue the user is about to ask to delete.
    if (e.button !== 0) return;
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
        const dragging = preview?.slot === cue.slot;
        // Past the right-hand edge the flag would be clipped by the
        // waveform's own overflow, so it folds to the other side of its line.
        //
        // The threshold is a long way out (0.94 rather than 0.86) because
        // every flip is a visible jump, and while panning a flag sitting
        // near the boundary flipped back and forth on consecutive frames.
        // Better to fold late and clip a few pixels than to flicker.
        const flagLeft = frac > 0.94;

        return (
          <button
            key={cue.id}
            type="button"
            aria-label={`Cue ${cue.slot + 1}, ${formatTime(ms / 1000)}`}
            title={`CUE ${cue.slot + 1} — ${CUE_KIND_LABELS[cue.kind]} — drag to move`}
            style={{ left: `${frac * 100}%`, color }}
            onPointerDown={(e) => onPointerDown(e, cue)}
            onPointerMove={onPointerMove}
            onPointerUp={(e) => onPointerUp(e, cue)}
            onContextMenu={(e) => onContextMenu(e, cue)}
            onPointerCancel={() => {
              drag.current = null;
              setDragging(null);
              setPreview(null);
            }}
            className={`absolute top-0 h-full w-3 -translate-x-1/2 cursor-ew-resize touch-none bg-transparent ${
              dragging ? 'z-30' : 'z-20'
            }`}
          >
            {/* One pixel wide at every zoom — it is the thing being aimed at,
                and a 2px line is 2px of ambiguity. */}
            <span
              className={`pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-current ${
                dragging ? 'opacity-100' : 'opacity-90'
              }`}
            />
            <span
              style={{ borderColor: color }}
              className={`mono absolute top-0 flex h-[17px] items-center whitespace-nowrap rounded-sm bg-gray-700/95 px-1.5 text-[10px] font-medium tracking-wide text-keystone-white shadow-sm ${
                flagLeft ? 'right-1/2 border-r-2' : 'left-1/2 border-l-2'
              } ${dragging ? 'bg-gray-600' : 'hover:bg-gray-600'}`}
            >
              CUE {cue.slot + 1}
            </span>
            {/* While dragging, where it would land. Placing a cue accurately
                means reading the number rather than estimating it. */}
            {dragging && (
              <span className="mono pointer-events-none absolute bottom-0 left-1/2 -translate-x-1/2 whitespace-nowrap rounded-sm bg-current px-1 py-0.5 text-[10px] leading-none text-[color:var(--color-on-accent)]">
                {formatTime(ms / 1000)}
              </span>
            )}
          </button>
        );
      })}
    </>
  );
}

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
  /** The waveform's pixel width, already measured by the parent's
      ResizeObserver. Needed to tell whether two flags would overlap, which is
      a question about pixels and not about time. */
  widthPx: number;
  onSeek: (seconds: number) => void;
  onContextMenu: (e: React.MouseEvent, cue: CuePoint) => void;
}

/** Below this, a pointerup is a click to jump rather than the end of a drag. */
const DRAG_SLOP_PX = 3;

/**
 * Roughly how wide a flag is: the dot, `CUE 8`, padding either side.
 *
 * An estimate rather than a measurement on purpose. Measuring would mean
 * rendering every flag, reading its box and re-rendering with the result —
 * two passes per frame while panning, to decide something that only has to be
 * approximately right. Over-estimating costs a lane that was not strictly
 * needed; under-estimating lets two labels touch, which is the bug.
 */
const FLAG_WIDTH_PX = 58;
/** Lane height: the flag's own 17px plus a hairline of air. */
const LANE_PX = 19;
/** How many lanes before flags start again at the top. Eight cues never need
    more than this in practice, and a flag pushed too far down leaves the
    waveform. */
const MAX_LANES = 3;

/**
 * CUE 1..8, drawn on the waveform as flags.
 *
 * A flag rather than a bare tick: the label hangs off the top, to the side of
 * the line, so the line itself stays one hairline wide and keeps marking the
 * exact sample while the name is still readable. A numbered box centred on the
 * line — which is what this was — hid the position it was pointing at behind
 * its own badge.
 *
 * Both the flag and the line are neutral. The line used to carry the cue's
 * own colour, and the engine's palette for a detected cue runs through the
 * same greens the waveform is drawn in — so a marker sitting on a loud bar
 * disappeared into it, which is the one thing a position marker may not do.
 * Grey is legible against every part of the waveform because nothing in the
 * waveform is grey. The cue's colour survives as a dot on its flag, where
 * there is a flat backdrop to read it against.
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
  widthPx,
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

  // Which lane each flag sits in, so two cues close together stack instead of
  // printing over each other.
  //
  // Cues are not laid out in slot order — they are wherever the track puts
  // them — so this works over a copy sorted by position. Greedy first-fit:
  // walk left to right and drop each flag in the topmost lane whose last flag
  // has already ended. That is enough because the flags are all the same
  // height and are processed in order of their left edge.
  const laneOf = new Map<string, number>();
  {
    // In fractions, because that is what the flags are positioned in.
    const flagFrac = widthPx > 0 ? FLAG_WIDTH_PX / widthPx : 0;

    const placed = cues
      .map((cue) => {
        const ms = preview?.slot === cue.slot ? preview.ms : cue.positionMs;
        const frac = (ms / 1000 - win.startS) / spanS;
        // A flag near the right edge folds to the left of its line, so the
        // span it occupies is on the other side. Same test as `flagLeft`.
        const left = frac > 0.94 ? frac - flagFrac : frac;
        return { id: cue.id, left };
      })
      // By left edge, not by position: first-fit only gives the topmost free
      // lane if the flags arrive in the order their boxes start, and a folded
      // flag starts a whole box earlier than its line.
      .sort((a, b) => a.left - b.left);

    // The right edge of the last flag placed in each lane.
    const laneEnds: number[] = [];

    for (const { id, left } of placed) {
      let lane = laneEnds.findIndex((end) => end <= left);
      if (lane === -1) {
        lane = laneEnds.length < MAX_LANES ? laneEnds.length : 0;
      }
      laneEnds[lane] = left + flagFrac;
      laneOf.set(id, lane);
    }
  }

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
            style={{ left: `${frac * 100}%` }}
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
              className={`pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 ${
                dragging ? 'bg-keystone-white' : 'bg-gray-300/80'
              }`}
            />
            <span
              style={{ top: (laneOf.get(cue.id) ?? 0) * LANE_PX }}
              className={`mono absolute flex h-[17px] items-center gap-1 whitespace-nowrap rounded-sm bg-gray-700/95 px-1.5 text-[10px] font-medium tracking-wide text-keystone-white shadow-sm ${
                flagLeft ? 'right-1/2' : 'left-1/2'
              } ${dragging ? 'bg-gray-600' : 'hover:bg-gray-600'}`}
            >
              {/* The cue's own colour, on a flat backdrop where it reads. */}
              <span
                aria-hidden
                style={{ background: color }}
                className="h-1.5 w-1.5 shrink-0 rounded-full"
              />
              CUE {cue.slot + 1}
            </span>
            {/* While dragging, where it would land. Placing a cue accurately
                means reading the number rather than estimating it. */}
            {dragging && (
              <span className="mono pointer-events-none absolute bottom-0 left-1/2 -translate-x-1/2 whitespace-nowrap rounded-sm bg-keystone-white px-1 py-0.5 text-[10px] leading-none text-keystone-black">
                {formatTime(ms / 1000)}
              </span>
            )}
          </button>
        );
      })}
    </>
  );
}

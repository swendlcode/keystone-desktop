import { createPortal } from 'react-dom';
import { EnergyValue, TempoValue } from '../library/TrackValues';
import { KeyResult } from '../library/KeyResult';
import { useSettings } from '../../hooks/useSettings';
import { useDragStore } from '../../stores/dragStore';

/** Pointer-to-corner offset, so the card sits beside the cursor, not under it. */
const OFFSET_X = 14;
const OFFSET_Y = 10;

/**
 * What the pointer is carrying, drawn next to it.
 *
 * HTML5 drag-and-drop could not do this. `setDragImage` takes a snapshot at
 * `dragstart` and nothing afterwards, so the image cannot react to what it is
 * over, and on a dark theme the browser's default image is a washed-out
 * screenshot of the row. A component following the pointer can show whatever
 * the decision needs.
 *
 * What it needs is the key, the energy and the tempo. Filing a track into a
 * crate is a judgement about how it will mix, and until this existed the
 * drag covered the row whose numbers you were reading — the gesture hid the
 * only information that justified it. The cells are the same components the
 * table and the player bar use, so an uncertain key still reads as uncertain
 * here.
 *
 * `pointer-events-none` is load-bearing: the hit test is
 * `document.elementFromPoint`, and a card under the cursor would be the
 * answer to every one of them.
 */
export function DragGhost() {
  const payload = useDragStore((s) => s.payload);
  const x = useDragStore((s) => s.x);
  const y = useDragStore((s) => s.y);
  const over = useDragStore((s) => s.overId != null);
  const { data: settings } = useSettings();

  if (!payload || payload.kind !== 'tracks') return null;

  const { lead, trackIds } = payload;
  const extra = trackIds.length - 1;

  return createPortal(
    <div
      aria-hidden
      style={{ left: x + OFFSET_X, top: y + OFFSET_Y }}
      className={`pointer-events-none fixed z-[100] max-w-[280px] rounded-lg border bg-gray-900/95 px-3 py-2 shadow-2xl backdrop-blur-sm transition-colors ${
        over
          ? 'border-keystone-lime/70 ring-1 ring-keystone-lime/40'
          : 'border-gray-700'
      }`}
    >
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-xs font-medium text-keystone-white">
          {lead.title || lead.path.split('/').pop()}
        </span>
        {extra > 0 && (
          <span className="mono tnum shrink-0 rounded bg-keystone-lime px-1.5 py-0.5 text-[10px] leading-none text-[color:var(--color-on-accent)]">
            +{extra}
          </span>
        )}
      </div>

      {lead.artist && (
        <p className="mt-0.5 truncate text-[11px] text-gray-400">{lead.artist}</p>
      )}

      <div className="mt-1.5 flex items-center gap-2.5 text-[11px]">
        <KeyResult
          track={lead}
          notation={settings?.keyNotation ?? 'camelot'}
          pad={settings?.padCamelotNumbers ?? false}
        />
        <span aria-hidden className="h-3 w-px bg-gray-700" />
        <EnergyValue track={lead} />
        <span aria-hidden className="h-3 w-px bg-gray-700" />
        <TempoValue track={lead} decimals={settings?.tempoDecimals ?? 0} />
      </div>
    </div>,
    document.body,
  );
}

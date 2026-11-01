import { useMemo } from 'react';
import { WheelSegment, type SegmentState } from './WheelSegment';
import { CENTRE, INNER_RING, VIEW_BOX } from './wheelGeometry';
import {
  CAMELOT_CODES,
  camelotAngle,
  compatibleCodes,
  parseCamelot,
  type CamelotKey,
} from '../../utils/camelot';

interface CamelotWheelProps {
  /** The filtered key, e.g. `8A`. */
  selected: string | null;
  /** The playing track's key. Lights the accent arc. */
  playing: string | null;
  /** Per-segment track counts. A missing code counts as zero. */
  counts: Map<string, number>;
  /** Whether the selection's three neighbours are lit as part of the filter. */
  includeCompatible: boolean;
  onSelect: (code: string) => void;
}

/**
 * The Camelot wheel: 1A-12A inside, 1B-12B outside, 1 at the top, clockwise.
 *
 * Controlled on purpose. It reads no store, which is what lets the same
 * component be dropped into a song-info panel or a future deck view at a
 * different size with a different selection, and is also why the geometry is
 * one fixed viewBox scaled by CSS rather than a function of a pixel size prop.
 *
 * The centre is HTML over the SVG rather than `<text>` plus a `<rect>` hit
 * area, so the readout in the hole gets the house type styles for free.
 *
 * It says what the state is and does not act on it. Clearing the filter used
 * to be a word inside the hole, under the key it was about to throw away:
 * small, unlabelled as a control, and in the one place a user looks to read
 * the current selection rather than to change it. `WheelPanel` puts a real
 * CLEAR FILTER button under the wheel instead, which is both findable and
 * what every other Camelot wheel does.
 */
export function CamelotWheel({
  selected,
  playing,
  counts,
  includeCompatible,
  onSelect,
}: CamelotWheelProps) {
  const compatible = useMemo(
    () =>
      new Set(
        selected && includeCompatible ? compatibleCodes(selected) : [],
      ),
    [selected, includeCompatible],
  );

  const stateOf = (code: string): SegmentState => {
    if (code === selected) return 'selected';
    if ((counts.get(code) ?? 0) === 0) return 'empty';
    if (compatible.has(code)) return 'compatible';
    return 'idle';
  };

  return (
    <div className="relative mx-auto aspect-square w-full max-w-[260px]">
      <svg
        viewBox={`0 0 ${VIEW_BOX} ${VIEW_BOX}`}
        className="absolute inset-0 h-full w-full"
        aria-label="Camelot wheel"
      >
        {CAMELOT_CODES.map((code) => {
          const key = parseCamelot(code) as CamelotKey;
          return (
            <WheelSegment
              key={code}
              camelotKey={key}
              bearingDeg={camelotAngle(key)}
              state={stateOf(code)}
              playing={code === playing}
              count={counts.get(code) ?? 0}
              onSelect={onSelect}
            />
          );
        })}
        {/* Fills the hole so the centre readout never sits on the page
            background when the panel is over a different surface. */}
        <circle
          cx={CENTRE.x}
          cy={CENTRE.y}
          r={INNER_RING.innerRadius - 2}
          fill="var(--color-bg)"
        />
      </svg>

      <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-1">
        <WheelCentre selected={selected} playing={playing} />
      </div>
    </div>
  );
}

/**
 * What the hole in the middle says.
 *
 * Three states, in priority order: the filter, then the playing key, then
 * nothing. The filter wins over the playing key because it is the thing the
 * user can act on, and the accent arc on the ring already says where playback
 * is.
 */
function WheelCentre({
  selected,
  playing,
}: {
  selected: string | null;
  playing: string | null;
}) {
  if (selected) {
    return (
      <>
        <span className="mono tnum text-2xl leading-none text-keystone-white">
          {selected}
        </span>
        <span className="label">Filtered</span>
      </>
    );
  }

  if (playing) {
    return (
      <>
        <span className="mono tnum text-2xl leading-none text-keystone-lime">
          {playing}
        </span>
        <span className="label">Playing</span>
      </>
    );
  }

  return <span className="label">Key</span>;
}

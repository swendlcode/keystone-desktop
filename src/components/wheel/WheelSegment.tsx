import { camelotColorAlpha } from '../../utils/camelotColors';
import { formatCamelot, type CamelotKey } from '../../utils/camelot';
import {
  labelPoint,
  ringFor,
  segmentEdgePath,
  segmentPath,
} from './wheelGeometry';

/** How lit a segment is. Exactly one of these applies at a time. */
export type SegmentState = 'selected' | 'compatible' | 'idle' | 'empty';

interface WheelSegmentProps {
  camelotKey: CamelotKey;
  bearingDeg: number;
  state: SegmentState;
  /** Draws the accent arc on the segment's outer edge. */
  playing: boolean;
  count: number;
  onSelect: (code: string) => void;
}

/**
 * Fill opacity per state.
 *
 * The compatible step is deliberately a long way from idle: the whole claim of
 * the wheel is that you can see at a glance which three keys you can go to
 * next, and 0.2 against 0.3 does not survive a glance.
 */
const FILL_ALPHA: Record<SegmentState, number> = {
  selected: 1,
  compatible: 0.62,
  idle: 0.17,
  empty: 0.05,
};

/**
 * One wedge of the wheel.
 *
 * Selection is drawn as a full-saturation fill plus a foreground-coloured
 * outline, not as an accent outline: the accent is spent on the playing key,
 * which is the live fact. A selection the user made themselves is already
 * named in the middle of the wheel and does not need the loudest colour too.
 */
export function WheelSegment({
  camelotKey,
  bearingDeg,
  state,
  playing,
  count,
  onSelect,
}: WheelSegmentProps) {
  const code = formatCamelot(camelotKey);
  const ring = ringFor(camelotKey.letter);
  const label = labelPoint(bearingDeg, camelotKey.letter);
  const empty = state === 'empty';

  // The A ring is the dark half of each hue pair, so a filled A segment takes
  // the light foreground and a filled B segment the dark one. Unfilled
  // segments ignore both and use the theme's own text colour.
  const labelFill =
    state === 'selected'
      ? camelotKey.letter === 'A'
        ? 'var(--camelot-fg-a)'
        : 'var(--camelot-fg-b)'
      : 'currentColor';

  return (
    <g
      role="button"
      tabIndex={empty ? -1 : 0}
      aria-label={`${code}, ${count} tracks`}
      aria-pressed={state === 'selected'}
      className={
        empty
          ? 'text-gray-600'
          : 'cursor-pointer text-keystone-white outline-none focus-visible:text-keystone-lime'
      }
      onClick={() => !empty && onSelect(code)}
      onKeyDown={(e) => {
        if (empty) return;
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onSelect(code);
        }
      }}
    >
      <title>{`${code} — ${count}`}</title>
      <path
        d={segmentPath(bearingDeg, ring)}
        fill={camelotColorAlpha(code, FILL_ALPHA[state]) ?? 'none'}
        stroke={state === 'selected' ? 'var(--color-text)' : 'var(--color-bg)'}
        strokeWidth={state === 'selected' ? 2 : 1}
        vectorEffect="non-scaling-stroke"
      />
      {playing && (
        <path
          d={segmentEdgePath(bearingDeg, ring)}
          fill="none"
          stroke="var(--color-accent)"
          strokeWidth={3}
          strokeLinecap="round"
        />
      )}
      <text
        x={label.x}
        y={label.y}
        fill={labelFill}
        textAnchor="middle"
        dominantBaseline="central"
        className="mono pointer-events-none"
        fontSize={camelotKey.letter === 'A' ? 9.5 : 10.5}
        opacity={empty ? 0.45 : 1}
      >
        {code}
      </text>
    </g>
  );
}

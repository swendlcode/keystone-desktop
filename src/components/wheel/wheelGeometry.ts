/**
 * The wheel's arithmetic, kept apart from the component that draws it.
 *
 * Pure functions of their arguments, for the same reason `camelot.ts` is: the
 * segment path, the label position and the playing-key pointer all have to
 * agree about where `8A` is, and a geometry bug that only shows up as a
 * half-degree drift in one of the three is invisible in a screenshot.
 *
 * Angles everywhere are **bearings**: degrees clockwise from 12 o'clock, which
 * is what `camelotAngle` returns. The conversion to SVG's y-down plane happens
 * once, in `polar`, and nowhere else.
 */

export interface Point {
  x: number;
  y: number;
}

/** The ring radii, in viewBox units. See `VIEW_BOX`. */
export interface RingGeometry {
  innerRadius: number;
  outerRadius: number;
}

/**
 * The wheel is drawn in a fixed 240x240 viewBox and scaled by CSS.
 *
 * That is what "crisp at any size" means in practice: one set of numbers, no
 * breakpoints, and the browser rasterises the paths at whatever the sidebar's
 * current width happens to be.
 */
export const VIEW_BOX = 240;
export const CENTRE: Point = { x: VIEW_BOX / 2, y: VIEW_BOX / 2 };

/** Minor inside, major outside — SPEC.md §1. */
export const INNER_RING: RingGeometry = { innerRadius: 52, outerRadius: 82 };
export const OUTER_RING: RingGeometry = { innerRadius: 86, outerRadius: 116 };

/** Where the code sits inside each ring. */
export const INNER_LABEL_RADIUS = 67;
export const OUTER_LABEL_RADIUS = 101;

/** 12 segments, so 30 degrees each, less a gap so the edges read as separate. */
const SEGMENT_SPAN = 30;
const SEGMENT_GAP = 2.2;

/** Bearing -> a point on the SVG plane. The only place the y flip happens. */
export function polar(radius: number, bearingDeg: number, centre = CENTRE): Point {
  const rad = (bearingDeg * Math.PI) / 180;
  return {
    x: centre.x + radius * Math.sin(rad),
    y: centre.y - radius * Math.cos(rad),
  };
}

/**
 * The `d` for one segment: an annulus sector centred on `bearingDeg`.
 *
 * `sweep-flag` is 1 on the outer edge and 0 on the inner one because the path
 * runs clockwise out and anticlockwise back; getting that pair wrong draws a
 * bow tie rather than a wedge. `large-arc-flag` is always 0 — a segment is
 * under 30 degrees, so it can never be the long way round.
 */
export function segmentPath(bearingDeg: number, ring: RingGeometry): string {
  const half = (SEGMENT_SPAN - SEGMENT_GAP) / 2;
  const from = bearingDeg - half;
  const to = bearingDeg + half;
  const { innerRadius: ri, outerRadius: ro } = ring;

  const o0 = polar(ro, from);
  const o1 = polar(ro, to);
  const i1 = polar(ri, to);
  const i0 = polar(ri, from);

  return [
    `M ${o0.x.toFixed(3)} ${o0.y.toFixed(3)}`,
    `A ${ro} ${ro} 0 0 1 ${o1.x.toFixed(3)} ${o1.y.toFixed(3)}`,
    `L ${i1.x.toFixed(3)} ${i1.y.toFixed(3)}`,
    `A ${ri} ${ri} 0 0 0 ${i0.x.toFixed(3)} ${i0.y.toFixed(3)}`,
    'Z',
  ].join(' ');
}

/**
 * The outer edge of a segment as a bare arc, for the now-playing marker.
 *
 * A stroked arc rather than an outline around the whole wedge, so "playing"
 * and "selected" cannot be mistaken for each other when a track is playing in
 * the key the user has also filtered to.
 */
export function segmentEdgePath(bearingDeg: number, ring: RingGeometry): string {
  const half = (SEGMENT_SPAN - SEGMENT_GAP) / 2;
  const a = polar(ring.outerRadius, bearingDeg - half);
  const b = polar(ring.outerRadius, bearingDeg + half);
  return `M ${a.x.toFixed(3)} ${a.y.toFixed(3)} A ${ring.outerRadius} ${ring.outerRadius} 0 0 1 ${b.x.toFixed(3)} ${b.y.toFixed(3)}`;
}

/** Centre of a segment's label. */
export function labelPoint(bearingDeg: number, letter: 'A' | 'B'): Point {
  return polar(letter === 'A' ? INNER_LABEL_RADIUS : OUTER_LABEL_RADIUS, bearingDeg);
}

export function ringFor(letter: 'A' | 'B'): RingGeometry {
  return letter === 'A' ? INNER_RING : OUTER_RING;
}

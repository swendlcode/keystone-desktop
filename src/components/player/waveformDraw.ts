import type { Waveform, WaveformLevel } from '../../types';

/**
 * Canvas drawing for the player's waveform, kept out of the component.
 *
 * None of this touches React, which is the point: the draw runs on every
 * `timeupdate` while a zoomed-in view follows the playhead, and anything in
 * here that caused a render would cost a reconcile 4 times a second for the
 * length of a track.
 */

export interface WaveWindow {
  /** Seconds. */
  startS: number;
  endS: number;
}

/** How many seconds fit across the canvas at a zoom step. */
export function waveSpan(durationS: number, zoom: number): number {
  if (!(durationS > 0)) return 1;
  return Math.min(durationS, durationS / 2 ** Math.max(0, zoom));
}

/**
 * The visible span at a zoom step, and where it sits.
 *
 * Zoom 0 is the whole track and each step halves the span. Clamping slides the
 * window rather than truncating it, so zooming in near the end of a track
 * still fills the canvas instead of leaving dead space.
 *
 * `centreS` is where the user has scrolled to; null means follow the playhead.
 * The two are separate because placing a cue accurately means looking at a bar
 * that is not the one playing — with the window pinned to the playhead the
 * only way to see another part of the track was to seek to it, which moves the
 * thing you were lining the cue up against.
 */
export function waveWindow(
  durationS: number,
  zoom: number,
  playheadS: number,
  centreS: number | null = null,
): WaveWindow {
  if (!(durationS > 0)) return { startS: 0, endS: 1 };
  const span = waveSpan(durationS, zoom);
  let startS = (centreS ?? playheadS) - span / 2;
  if (startS < 0) startS = 0;
  if (startS + span > durationS) startS = durationS - span;
  return { startS, endS: startS + span };
}

/**
 * The coarsest level that still has a bucket for every pixel.
 *
 * Coarsest-that-suffices rather than finest-available: the finest level of a
 * ten-minute track is millions of buckets, and reducing 400 of them into one
 * pixel column costs the same as reducing 4 and looks identical.
 */
export function pickLevel(
  wave: Waveform,
  window: WaveWindow,
  widthPx: number,
): number {
  const spanS = Math.max(window.endS - window.startS, 1e-6);
  for (let i = 0; i < wave.levels.length; i += 1) {
    const buckets = (spanS * wave.sampleRate) / wave.levels[i].samplesPerBucket;
    if (buckets >= widthPx) return i;
  }
  return wave.levels.length - 1;
}

interface DrawOptions {
  wave: Waveform;
  window: WaveWindow;
  playheadS: number;
  /**
   * DEVICE pixels — the canvas's real backing size, not its CSS size.
   *
   * This used to be the CSS size, with the context scaled by the device pixel
   * ratio to compensate. The compensation is what cost the detail: the column
   * loop ran once per CSS pixel, so on a 2x display every column was painted
   * two device pixels wide and half the resolution the canvas was already
   * paying for was thrown away. Drawing in device space is the whole
   * difference between a blocky waveform and a sharp one.
   */
  width: number;
  height: number;
  /** Per-pixel peaks — the sharp layer, where each transient is its own spike. */
  playedColor: string;
  unplayedColor: string;
  /** The smoothed shape behind it, which carries the arrangement. */
  playedGhostColor: string;
  unplayedGhostColor: string;
  centreColor: string;
}

/** Silence still reads as a line rather than as nothing. */
const MIN_THICKNESS = 1;

/**
 * Peaks stop short of the edges.
 *
 * A modern club master is limited to within a hair of full scale for minutes
 * at a time, so a waveform drawn edge to edge is a solid slab touching both
 * walls of its box — there is no way to tell a loud section from a louder one
 * because both are clipped by the frame rather than by the music.
 */
const HEADROOM = 0.88;

/**
 * Half-width, in device pixels, of the window the ghost layer maxes over.
 *
 * Wide enough to bridge the gaps between kicks, so the ghost reads as the
 * song's shape rather than as a blurred copy of the spikes in front of it;
 * narrow enough that a breakdown still collapses instead of being filled in
 * by the drop next to it.
 */
const GHOST_RADIUS_PX = 7;

/**
 * Half-width, in device pixels, of the trough the bright layer is measured
 * against.
 *
 * Small — a few pixels either side, which at full zoom-out is a fraction of
 * a beat. It has to be narrower than the gap between kicks, or the trough it
 * finds is another kick's and every hit measures as flat.
 */
const BEAT_RADIUS_PX = 4;

/**
 * How the bright layer is mixed from the sustained level and the transient.
 *
 * A modern club master is limited to within a hair of full scale, so peak
 * height says almost nothing — a breakdown and a drop draw the same
 * rectangle. What still moves is how far each column RISES above the quiet
 * just before it, which is exactly what a drum hit is. The bright layer is
 * mostly that rise, plus a little of the sustained level so the body does
 * not collapse to a line between hits.
 */
const SUSTAIN_SHARE = 0.3;
const TRANSIENT_GAIN = 1.6;

interface Envelope {
  tops: Float32Array;
  bottoms: Float32Array;
  ghostTops: Float32Array;
  ghostBottoms: Float32Array;
}

/**
 * The last envelope measured, keyed by everything it depends on.
 *
 * The shape is a function of the file, the window and the canvas size — and
 * emphatically NOT of the playhead. Now that the playhead advances once per
 * frame rather than four times a second, measuring it again every frame
 * would mean a sliding max over a few thousand columns sixty times a second
 * to produce an identical answer. One slot is enough: there is one waveform
 * on screen.
 */
let cachedKey = '';
let cachedEnvelope: Envelope | null = null;

/** Repaints the whole canvas. There is no partial-invalidation path. */
export function drawWaveform(
  ctx: CanvasRenderingContext2D,
  opts: DrawOptions,
): void {
  const { wave, window, width, height, playheadS } = opts;
  ctx.clearRect(0, 0, width, height);
  if (width <= 0 || height <= 0) return;

  const level = wave.levels[pickLevel(wave, window, width)];
  if (!level) return;

  const mid = height / 2;
  const spanS = Math.max(window.endS - window.startS, 1e-6);

  const key = `${wave.contentHash}|${window.startS}|${window.endS}|${width}|${height}`;
  if (key !== cachedKey || !cachedEnvelope) {
    cachedEnvelope = measureEnvelope(wave, level, window, width, mid);
    cachedKey = key;
  }
  const { tops, bottoms, ghostTops, ghostBottoms } = cachedEnvelope;

  // Clipped rather than decided per column, so the split lands on the exact
  // playhead instead of snapping to a pixel boundary.
  const playedPx = Math.min(Math.max(((playheadS - window.startS) / spanS) * width, 0), width);

  // Ghost behind, detail in front. The ghost is a max over a window, so it
  // bridges the gaps between kicks and draws the arrangement — the shape that
  // tells you where the breakdown is from across the room. The sharp layer
  // on top keeps every transient its own spike, which is what the ghost
  // would otherwise smear away.
  const layers = (ghostColor: string, detailColor: string) => {
    paintEnvelope(ctx, ghostTops, ghostBottoms, width, ghostColor);
    paintEnvelope(ctx, tops, bottoms, width, detailColor);
  };

  layers(opts.unplayedGhostColor, opts.unplayedColor);

  if (playedPx > 0) {
    ctx.save();
    ctx.beginPath();
    ctx.rect(0, 0, playedPx, height);
    ctx.clip();
    layers(opts.playedGhostColor, opts.playedColor);
    ctx.restore();
  }

  ctx.fillStyle = opts.centreColor;
  ctx.fillRect(0, Math.round(mid), width, 1);
}

/** Measures the four edge arrays. Independent of the playhead. */
function measureEnvelope(
  wave: Waveform,
  level: WaveformLevel,
  window: WaveWindow,
  width: number,
  mid: number,
): Envelope {
  const spanS = Math.max(window.endS - window.startS, 1e-6);
  const bucketsPerSecond = wave.sampleRate / level.samplesPerBucket;

  const hi = new Float32Array(width);
  const lo = new Float32Array(width);
  for (let x = 0; x < width; x += 1) {
    const t0 = window.startS + (x / width) * spanS;
    const t1 = window.startS + ((x + 1) / width) * spanS;
    const peak = bucketPeak(level, t0 * bucketsPerSecond, t1 * bucketsPerSecond);
    hi[x] = peak ? peak.max : 0;
    lo[x] = peak ? peak.min : 0;
  }

  // The ghost is the whole song: a max over a wide window, so it bridges the
  // gaps between hits and draws the arrangement.
  const ghostHi = slidingMax(hi, GHOST_RADIUS_PX);
  const ghostLo = slidingMin(lo, GHOST_RADIUS_PX);

  // The bright layer is the drums: each column measured against the quiet
  // immediately around it rather than against full scale.
  const floorHi = slidingMin(hi, BEAT_RADIUS_PX);
  const floorLo = slidingMax(lo, BEAT_RADIUS_PX);

  const scale = mid * HEADROOM;
  const tops = new Float32Array(width);
  const bottoms = new Float32Array(width);
  const ghostTops = new Float32Array(width);
  const ghostBottoms = new Float32Array(width);

  for (let x = 0; x < width; x += 1) {
    // Never taller than the real peak: a transient is being emphasised, not
    // invented, and a bright layer poking out of its own ghost would be the
    // drawing contradicting itself.
    const beatHi = Math.min(
      hi[x],
      floorHi[x] * SUSTAIN_SHARE + (hi[x] - floorHi[x]) * TRANSIENT_GAIN,
    );
    const beatLo = Math.max(
      lo[x],
      floorLo[x] * SUSTAIN_SHARE + (lo[x] - floorLo[x]) * TRANSIENT_GAIN,
    );

    let top = mid - beatHi * scale;
    let bottom = mid - beatLo * scale;
    if (bottom - top < MIN_THICKNESS) {
      top = mid - MIN_THICKNESS / 2;
      bottom = mid + MIN_THICKNESS / 2;
    }
    tops[x] = top;
    bottoms[x] = bottom;
    ghostTops[x] = mid - ghostHi[x] * scale;
    ghostBottoms[x] = mid - ghostLo[x] * scale;
  }

  return { tops, bottoms, ghostTops, ghostBottoms };
}

/**
 * Running maximum over a window of +/- `radius` columns.
 *
 * Plain nested loops. The window is a handful of pixels wide and this runs on
 * a few thousand columns four times a second, so a monotonic deque would be
 * more code to read for no measurable difference.
 */
function slidingMax(src: Float32Array, radius: number): Float32Array {
  const out = new Float32Array(src.length);
  for (let i = 0; i < src.length; i += 1) {
    const from = Math.max(0, i - radius);
    const to = Math.min(src.length - 1, i + radius);
    let best = src[from];
    for (let j = from + 1; j <= to; j += 1) if (src[j] > best) best = src[j];
    out[i] = best;
  }
  return out;
}

function slidingMin(src: Float32Array, radius: number): Float32Array {
  const out = new Float32Array(src.length);
  for (let i = 0; i < src.length; i += 1) {
    const from = Math.max(0, i - radius);
    const to = Math.min(src.length - 1, i + radius);
    let best = src[from];
    for (let j = from + 1; j <= to; j += 1) if (src[j] < best) best = src[j];
    out[i] = best;
  }
  return out;
}

/**
 * The envelope as one filled shape: the top edge left to right, the bottom
 * edge back again.
 *
 * Independent 1px rects per column leave a visible stair wherever the
 * amplitude moves quickly, because nothing joins one column's top to the
 * next. A single closed path joins them, which is what makes a loud passage
 * read as a shape instead of a comb.
 */
function paintEnvelope(
  ctx: CanvasRenderingContext2D,
  tops: Float32Array,
  bottoms: Float32Array,
  width: number,
  color: string,
): void {
  ctx.beginPath();
  ctx.moveTo(0, tops[0]);
  for (let x = 1; x < width; x += 1) ctx.lineTo(x, tops[x]);
  for (let x = width - 1; x >= 0; x -= 1) ctx.lineTo(x, bottoms[x]);
  ctx.closePath();
  ctx.fillStyle = color;
  ctx.fill();
}

interface ColumnPeak {
  /** Loudest excursion in the column — the outer envelope. */
  min: number;
  max: number;
}

/** min/max across a fractional bucket range, or null when it is out of range. */
function bucketPeak(
  level: WaveformLevel,
  fromBucket: number,
  toBucket: number,
): ColumnPeak | null {
  const n = level.max.length;
  if (n === 0 || fromBucket >= n || toBucket <= 0) return null;

  const first = Math.floor(fromBucket);
  const last = Math.ceil(toBucket) - 1;

  // Zoomed in past the finest level, so one bucket covers several pixels.
  // Repeating its value would draw a staircase of flat runs; stepping towards
  // the next bucket draws the slope that is actually there. The finest level
  // is 128 samples, about 2.9 ms, and the deepest zoom is well under 1 ms per
  // pixel — so this is the common case at the zoom used to place a cue.
  if (last <= first) {
    const i = Math.min(n - 1, Math.max(0, first));
    const j = Math.min(n - 1, i + 1);
    const t = Math.min(1, Math.max(0, fromBucket - first));
    const min = level.min[i] + (level.min[j] - level.min[i]) * t;
    const max = level.max[i] + (level.max[j] - level.max[i]) * t;
    return { min, max };
  }

  const a = Math.max(0, first);
  const b = Math.min(n - 1, last);
  if (a > b) return null;

  let min = 0;
  let max = 0;
  for (let i = a; i <= b; i += 1) {
    if (level.min[i] < min) min = level.min[i];
    if (level.max[i] > max) max = level.max[i];
  }
  return { min, max };
}

/** Theme colours, read from the document rather than hardcoded. */
export function waveColors(): Pick<
  DrawOptions,
  | 'playedColor'
  | 'unplayedColor'
  | 'playedGhostColor'
  | 'unplayedGhostColor'
  | 'centreColor'
> {
  const style = getComputedStyle(document.documentElement);
  const rgb = (token: string, alpha?: number) => {
    const triplet = style.getPropertyValue(token).trim();
    return alpha == null ? `rgb(${triplet})` : `rgb(${triplet} / ${alpha})`;
  };
  return {
    // Detail bright, ghost dim. The eye reads the spikes as the beat and the
    // dim shape behind them as the song.
    playedColor: rgb('--keystone-lime'),
    playedGhostColor: rgb('--keystone-lime', 0.22),
    unplayedColor: rgb('--gray-400'),
    unplayedGhostColor: rgb('--gray-700'),
    centreColor: rgb('--gray-700'),
  };
}

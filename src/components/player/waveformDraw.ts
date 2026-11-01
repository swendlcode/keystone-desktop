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
  playedColor: string;
  unplayedColor: string;
  centreColor: string;
}

/** Silence still reads as a line rather than as nothing. */
const MIN_THICKNESS = 1;

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
  const bucketsPerSecond = wave.sampleRate / level.samplesPerBucket;

  // The envelope is measured once and painted twice, so the played and
  // unplayed halves are the same shape rather than two shapes that have to
  // agree at the seam.
  const tops = new Float32Array(width);
  const bottoms = new Float32Array(width);

  for (let x = 0; x < width; x += 1) {
    const t0 = window.startS + (x / width) * spanS;
    const t1 = window.startS + ((x + 1) / width) * spanS;
    const peak = bucketPeak(level, t0 * bucketsPerSecond, t1 * bucketsPerSecond);
    if (!peak) {
      tops[x] = mid;
      bottoms[x] = mid;
      continue;
    }
    let top = mid - peak.max * mid;
    let bottom = mid - peak.min * mid;
    if (bottom - top < MIN_THICKNESS) {
      top = mid - MIN_THICKNESS / 2;
      bottom = mid + MIN_THICKNESS / 2;
    }
    tops[x] = top;
    bottoms[x] = bottom;
  }

  paintEnvelope(ctx, tops, bottoms, width, opts.unplayedColor);

  // Clipped rather than decided per column, so the split lands on the exact
  // playhead instead of snapping to a pixel boundary.
  const playedPx = ((playheadS - window.startS) / spanS) * width;
  if (playedPx > 0) {
    ctx.save();
    ctx.beginPath();
    ctx.rect(0, 0, Math.min(playedPx, width), height);
    ctx.clip();
    paintEnvelope(ctx, tops, bottoms, width, opts.playedColor);
    ctx.restore();
  }

  ctx.fillStyle = opts.centreColor;
  ctx.fillRect(0, Math.round(mid), width, 1);
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

/** min/max across a fractional bucket range, or null when it is out of range. */
function bucketPeak(
  level: WaveformLevel,
  fromBucket: number,
  toBucket: number,
): { min: number; max: number } | null {
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
    return {
      min: level.min[i] + (level.min[j] - level.min[i]) * t,
      max: level.max[i] + (level.max[j] - level.max[i]) * t,
    };
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
  'playedColor' | 'unplayedColor' | 'centreColor'
> {
  const style = getComputedStyle(document.documentElement);
  const rgb = (token: string) => `rgb(${style.getPropertyValue(token).trim()})`;
  return {
    playedColor: rgb('--keystone-lime'),
    unplayedColor: rgb('--gray-600'),
    centreColor: rgb('--gray-700'),
  };
}

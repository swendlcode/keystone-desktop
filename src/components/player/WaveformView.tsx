import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { CueMarkers } from './CueMarkers';
import { drawWaveform, waveColors, waveSpan, waveWindow } from './waveformDraw';
import { useWaveform } from '../../hooks/useAnalysis';
import { useCuePoints } from '../../hooks/useCues';
import { useEffectiveSettings } from '../../hooks/useSettings';
import { MAX_ZOOM, usePlayerStore } from '../../stores/playerStore';
import type { Track } from '../../types';

interface WaveformViewProps {
  track: Track;
  onSeek: (seconds: number) => void;
}

const HEIGHT = 72;

/**
 * The full-width waveform with its eight cue markers.
 *
 * The peaks are a pyramid on disk keyed by content hash, so they arrive once
 * and never change for a given file; the canvas is repainted on every
 * `timeupdate` only because the played/unplayed split and, past zoom 0, the
 * visible window both follow the playhead.
 */
export function WaveformView({ track, onSeek }: WaveformViewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: HEIGHT });

  const { data: wave } = useWaveform(track.id);
  const { data: cues = [] } = useCuePoints(track.id);
  const settings = useEffectiveSettings();
  const currentTime = usePlayerStore((s) => s.currentTime);
  const storeDuration = usePlayerStore((s) => s.duration);
  const zoom = usePlayerStore((s) => s.zoom);
  const viewCentreS = usePlayerStore((s) => s.viewCentreS);

  const durationS =
    storeDuration || (wave?.durationMs ?? track.durationMs ?? 0) / 1000;
  const win = waveWindow(durationS, zoom, currentTime, viewCentreS);

  useWaveformWheel(boxRef, durationS);

  // A manual pan holds until it stops being useful. Once playback has run out
  // of the window the user scrolled to, following the playhead is what they
  // want again — and the alternative, a view that never comes back on its own,
  // looks like the waveform has frozen.
  useEffect(() => {
    if (viewCentreS == null) return;
    if (currentTime >= win.startS && currentTime <= win.endS) return;
    usePlayerStore.getState().setViewCentre(null);
  }, [currentTime, viewCentreS, win.startS, win.endS]);

  useLayoutEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    const ro = new ResizeObserver(([entry]) => {
      setSize({
        width: Math.floor(entry.contentRect.width),
        height: Math.floor(entry.contentRect.height),
      });
    });
    ro.observe(box);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !wave || size.width === 0) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // Back the canvas at device resolution and draw in device pixels too.
    // Scaling the context by the ratio and then looping once per CSS pixel —
    // which is what this did — paints every column two device pixels wide and
    // throws away exactly the resolution the larger backing store bought.
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(size.width * dpr);
    canvas.height = Math.round(size.height * dpr);
    ctx.setTransform(1, 0, 0, 1, 0, 0);

    drawWaveform(ctx, {
      wave,
      window: win,
      playheadS: currentTime,
      width: canvas.width,
      height: canvas.height,
      ...waveColors(),
    });
  }, [wave, size, win.startS, win.endS, currentTime]);

  const seekFromClientX = (clientX: number) => {
    const rect = boxRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return;
    const frac = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    onSeek(win.startS + frac * (win.endS - win.startS));
  };

  const playheadFrac =
    (currentTime - win.startS) / Math.max(win.endS - win.startS, 1e-6);

  // The Display Options toggle hides what the detector proposed; a cue the
  // user placed or dragged is theirs and stays visible either way.
  const visibleCues =
    settings?.showDetectedCuePoints === false
      ? cues.filter((c) => c.source === 'user')
      : cues;

  return (
    <div
      ref={boxRef}
      style={{ height: HEIGHT }}
      onPointerDown={(e) => seekFromClientX(e.clientX)}
      title={
        zoom > 0
          ? 'Scroll to pan · \u2318 scroll to zoom'
          : '\u2318 scroll to zoom in'
      }
      className="relative w-full cursor-col-resize overflow-hidden rounded-md bg-gray-800"
    >
      <canvas ref={canvasRef} className="block h-full w-full" />
      {!wave && <WavePlaceholder track={track} />}
      {playheadFrac >= 0 && playheadFrac <= 1 && (
        <span
          style={{ left: `${playheadFrac * 100}%` }}
          className="pointer-events-none absolute inset-y-0 w-px bg-keystone-white"
        />
      )}
      <CueMarkers
        cues={visibleCues}
        window={win}
        containerRef={boxRef}
        onSeek={onSeek}
      />
    </div>
  );
}

/**
 * Scroll to pan, Cmd / Ctrl scroll to zoom about the pointer.
 *
 * A native listener rather than React's `onWheel`, because React registers
 * wheel handlers passively and a passive handler cannot call
 * `preventDefault()` — without which Cmd-scroll is the webview's page zoom and
 * the whole window scales instead of the waveform.
 *
 * Zoom is anchored on the pointer, not on the centre: the second under the
 * cursor stays under the cursor. That is what makes it usable for placing a
 * cue, because the thing you are zooming towards is the thing you are
 * pointing at.
 */
function useWaveformWheel(
  boxRef: React.RefObject<HTMLDivElement>,
  durationS: number,
) {
  const durationRef = useRef(durationS);
  durationRef.current = durationS;

  useEffect(() => {
    const box = boxRef.current;
    if (!box) return;

    const onWheel = (e: WheelEvent) => {
      const total = durationRef.current;
      if (!(total > 0)) return;
      const rect = box.getBoundingClientRect();
      if (rect.width === 0) return;

      const store = usePlayerStore.getState();
      const frac = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
      const win = waveWindow(total, store.zoom, store.currentTime, store.viewCentreS);
      const span = win.endS - win.startS;

      if (e.metaKey || e.ctrlKey) {
        e.preventDefault();
        const next = Math.max(
          0,
          Math.min(MAX_ZOOM, store.zoom + (e.deltaY < 0 ? 1 : -1)),
        );
        if (next === store.zoom) return;
        const anchorS = win.startS + frac * span;
        store.setZoom(next);
        // Zoom 0 is the whole track, so there is nothing left to pan and the
        // centre goes back to following the playhead.
        store.setViewCentre(
          next === 0 ? null : anchorS + (0.5 - frac) * waveSpan(total, next),
        );
        return;
      }

      // Nothing to pan to at zoom 0, and swallowing the event there would eat
      // a scroll the user meant for the list below.
      if (store.zoom === 0) return;
      e.preventDefault();
      // Trackpads send a two-axis delta; a mouse wheel only sends deltaY. Take
      // whichever axis the gesture actually moved on.
      const delta =
        Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
      store.setViewCentre(
        (win.startS + win.endS) / 2 + (delta / rect.width) * span,
      );
    };

    box.addEventListener('wheel', onWheel, { passive: false });
    return () => box.removeEventListener('wheel', onWheel);
  }, [boxRef]);
}

/**
 * What the strip says while there is nothing to draw.
 *
 * Three different facts, named rather than merged into one spinner: a track
 * the queue has not reached, a track being analysed, and a track that could
 * not be read are three different things for the user to do next.
 */
function WavePlaceholder({ track }: { track: Track }) {
  const text =
    track.indexStatus === 'error'
      ? 'Could not read'
      : track.indexStatus === 'pending'
        ? 'Queued'
        : track.analyzedAt == null
          ? 'Analysing'
          : 'No waveform';

  return (
    <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
      <span className="absolute inset-x-0 top-1/2 h-px bg-gray-700" />
      <span
        className={`label relative px-2 ${
          track.indexStatus === 'error' ? 'text-amber-400' : ''
        }`}
      >
        {text}
      </span>
    </div>
  );
}

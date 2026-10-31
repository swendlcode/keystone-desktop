import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { CueMarkers } from './CueMarkers';
import { drawWaveform, waveColors, waveWindow } from './waveformDraw';
import { useWaveform } from '../../hooks/useAnalysis';
import { useCuePoints } from '../../hooks/useCues';
import { useEffectiveSettings } from '../../hooks/useSettings';
import { usePlayerStore } from '../../stores/playerStore';
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

  const durationS =
    storeDuration || (wave?.durationMs ?? track.durationMs ?? 0) / 1000;
  const win = waveWindow(durationS, zoom, currentTime);

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

    // Back the canvas at device resolution and keep drawing in CSS pixels.
    // Without this the 1px columns land between device pixels and the whole
    // waveform reads as a grey smear on a retina display.
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(size.width * dpr);
    canvas.height = Math.round(size.height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    drawWaveform(ctx, {
      wave,
      window: win,
      playheadS: currentTime,
      width: size.width,
      height: size.height,
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

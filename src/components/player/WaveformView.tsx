import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { CueMarkers } from './CueMarkers';
import { CueRegions } from './CueRegions';
import { WaveformMenu, type WaveformMenuAnchor } from './WaveformMenu';
import { WaveformScrollbar } from './WaveformScrollbar';
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

const HEIGHT = 84;

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
  /** Pointer position across the strip, 0-1, or null when it is elsewhere. */
  const [hoverFrac, setHoverFrac] = useState<number | null>(null);
  const [menu, setMenu] = useState<WaveformMenuAnchor | null>(null);

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

  // Re-attach to the playhead once playback has left the window the user
  // panned to — but NOT while the pointer is over the waveform.
  //
  // Without that second condition this fought every zoom. Cmd-scrolling in on
  // a section away from the playhead puts the playhead outside the window by
  // definition, so this effect fired on the same frame and yanked the view
  // back to the playhead: the zoom appeared to land somewhere other than
  // under the cursor, at random, depending on where playback happened to be.
  // A pointer over the strip means the user is working with it, and the view
  // is theirs until they look away.
  useEffect(() => {
    if (viewCentreS == null || hoverFrac != null) return;
    if (currentTime >= win.startS && currentTime <= win.endS) return;
    usePlayerStore.getState().setViewCentre(null);
  }, [currentTime, viewCentreS, hoverFrac, win.startS, win.endS]);

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

  const fracFromClientX = (clientX: number): number | null => {
    const rect = boxRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return null;
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  };

  /**
   * Only re-render when the pointer has actually moved somewhere different.
   *
   * `pointermove` fires far faster than the screen refreshes, and each one
   * was a render of the markers, the regions and a canvas repaint — which is
   * what made dragging across the strip feel like it was stuttering rather
   * than tracking.
   */
  const trackHover = (clientX: number) => {
    const next = fracFromClientX(clientX);
    setHoverFrac((prev) =>
      prev != null && next != null && Math.abs(prev - next) < 0.002 ? prev : next,
    );
  };

  const openMenu = (e: React.MouseEvent, cue: (typeof cues)[number] | null) => {
    e.preventDefault();
    e.stopPropagation();
    const frac = fracFromClientX(e.clientX);
    if (frac == null) return;
    setMenu({
      x: e.clientX,
      y: e.clientY,
      positionMs: (win.startS + frac * (win.endS - win.startS)) * 1000,
      cue,
    });
  };

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
      onPointerDown={(e) => e.button === 0 && seekFromClientX(e.clientX)}
      onPointerMove={(e) => trackHover(e.clientX)}
      onPointerLeave={() => setHoverFrac(null)}
      onContextMenu={(e) => openMenu(e, null)}
      // No `title` here. A native tooltip on the strip itself pops up in the
      // middle of the waveform after a second of hovering and covers the part
      // the pointer is aimed at — which is the part being read. The scroll and
      // zoom gestures are announced on the zoom control instead.
      aria-label="Waveform — scroll to zoom, scroll sideways to pan, right-click to add a cue"
      className="relative w-full cursor-col-resize overflow-hidden rounded-md bg-gray-800"
    >
      <canvas ref={canvasRef} className="block h-full w-full" />
      {!wave && <WavePlaceholder track={track} />}
      {playheadFrac >= 0 && playheadFrac <= 1 && (
        // Red, and the only red in the player. The playhead was white, which
        // is also the colour of the loudest part of the waveform it crosses,
        // so it disappeared into exactly the material you watch it against.
        //
        // Positioned with `translate3d` rather than `left` so it moves on the
        // compositor: this now updates every frame, and animating `left`
        // relayouts the strip sixty times a second.
        <span
          style={{ transform: `translate3d(${playheadFrac * size.width}px, 0, 0)` }}
          className="pointer-events-none absolute inset-y-0 left-0 z-30 w-px bg-red-500 will-change-transform"
        >
          {/* The head. A line alone is hard to grab visually when the
              waveform behind it is dense; the marker is what the eye
              tracks. */}
          <span className="absolute -top-px left-1/2 h-0 w-0 -translate-x-1/2 border-x-[5px] border-t-[7px] border-x-transparent border-t-red-500" />
        </span>
      )}
      <CueRegions
        cues={visibleCues}
        window={win}
        durationS={durationS}
        hoverFrac={hoverFrac}
        onSeek={onSeek}
      />
      <CueMarkers
        cues={visibleCues}
        window={win}
        containerRef={boxRef}
        onSeek={onSeek}
        onContextMenu={(e, cue) => openMenu(e, cue)}
      />
      <WaveformScrollbar window={win} durationS={durationS} />
      {menu && (
        <WaveformMenu
          anchor={menu}
          trackId={track.id}
          cues={cues}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}

/**
 * Scroll up and down to zoom; scroll sideways to pan.
 *
 * The axis decides, not a modifier. On a trackpad the two gestures are
 * already distinct — a two-finger swipe up and a two-finger swipe sideways —
 * so spending a held key on the distinction asks for a third hand while
 * placing a cue. Cmd and Ctrl still zoom, because that is the reflex from
 * every other canvas and a mouse with one wheel has no sideways axis.
 *
 * A native listener rather than React's `onWheel`, because React registers
 * wheel handlers passively and a passive handler cannot call
 * `preventDefault()` — without which Cmd-scroll is the webview's page zoom
 * and the whole window scales instead of the waveform.
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

      // Sideways means pan — but only when the gesture is actually sideways.
      // A trackpad reports a little of both axes on almost every swipe, so
      // the dominant one wins rather than any horizontal movement at all
      // hijacking a vertical scroll.
      const sideways =
        Math.abs(e.deltaX) > Math.abs(e.deltaY) && !e.metaKey && !e.ctrlKey;

      if (sideways) {
        // Nothing to pan to at zoom 0: the whole track is already on screen.
        if (store.zoom === 0) return;
        e.preventDefault();
        store.setViewCentre(
          (win.startS + win.endS) / 2 + (e.deltaX / rect.width) * span,
        );
        return;
      }

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

import { Play, Stop } from '../ui/icons';
import { usePlayerStore } from '../../stores/playerStore';
import type { CuePoint } from '../../types';
import { formatTime } from '../../utils/formatters';
import type { WaveWindow } from './waveformDraw';

interface CueRegionsProps {
  cues: CuePoint[];
  window: WaveWindow;
  durationS: number;
  /** Pointer position inside the waveform, 0-1, or null when not hovering. */
  hoverFrac: number | null;
  onSeek: (seconds: number) => void;
}

/**
 * Play the section the pointer is over, from the cue that starts it.
 *
 * Cues cut a track into sections, and the thing a DJ does with a section is
 * audition it — but the only way to start one was to click its flag, which
 * seeks without playing, or to scrub to roughly the right place. One button,
 * following the pointer's section rather than one per section, because eight
 * play buttons across the waveform is eight things competing with the
 * waveform for the same glance.
 *
 * It starts at the section's cue, not at the pointer: the cue is where the
 * section begins, and a button that starts wherever you happened to be
 * hovering is a scrub with extra steps.
 */
export function CueRegions({
  cues,
  window: win,
  durationS,
  hoverFrac,
  onSeek,
}: CueRegionsProps) {
  const isPlaying = usePlayerStore((s) => s.isPlaying);
  const currentTime = usePlayerStore((s) => s.currentTime);

  if (hoverFrac == null || cues.length === 0) return null;

  const spanS = Math.max(win.endS - win.startS, 1e-6);
  const atS = win.startS + hoverFrac * spanS;

  // Section boundaries are the cues themselves, plus the ends of the track —
  // the run before the first cue and after the last are sections too.
  const marks = [0, ...cues.map((c) => c.positionMs / 1000), durationS].sort(
    (a, b) => a - b,
  );

  let startS = marks[0];
  let endS = marks[marks.length - 1];
  for (let i = 0; i < marks.length - 1; i += 1) {
    if (atS >= marks[i] && atS < marks[i + 1]) {
      startS = marks[i];
      endS = marks[i + 1];
      break;
    }
  }

  const left = (startS - win.startS) / spanS;
  const right = (endS - win.startS) / spanS;
  // Entirely off-screen, or too narrow at this zoom to put a button in.
  if (right <= 0 || left >= 1 || right - left < 0.04) return null;

  const centre = (Math.max(left, 0) + Math.min(right, 1)) / 2;

  // Playing *this* section — the transport is running and the playhead is
  // inside these bounds. A section elsewhere still offers play, because
  // pressing it means "go there", not "stop that".
  const playingHere =
    isPlaying && currentTime >= startS && currentTime < endS;

  const toggleSection = () => {
    if (playingHere) {
      usePlayerStore.getState().pause();
      return;
    }
    onSeek(startS);
    usePlayerStore.getState().resume();
  };

  return (
    <>
      {/* The section, lit faintly so its extent is visible before the click. */}
      <span
        style={{
          left: `${Math.max(left, 0) * 100}%`,
          width: `${(Math.min(right, 1) - Math.max(left, 0)) * 100}%`,
        }}
        className="pointer-events-none absolute inset-y-0 z-10 bg-keystone-white/[0.06]"
      />
      <button
        type="button"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={toggleSection}
        title={
          playingHere ? 'Stop' : `Play from ${formatTime(startS)}`
        }
        aria-label={
          playingHere
            ? 'Stop'
            : `Play the section starting at ${formatTime(startS)}`
        }
        style={{ left: `${centre * 100}%` }}
        className={`absolute top-1/2 z-20 flex h-7 w-7 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full transition-colors ${
          playingHere
            ? 'bg-keystone-lime text-[color:var(--color-on-accent)]'
            : 'bg-gray-900/85 text-keystone-white hover:bg-keystone-lime hover:text-[color:var(--color-on-accent)]'
        }`}
      >
        {playingHere ? (
          <Stop size={12} color="currentColor" variant="Linear" />
        ) : (
          <Play size={13} color="currentColor" variant="Linear" />
        )}
      </button>
    </>
  );
}

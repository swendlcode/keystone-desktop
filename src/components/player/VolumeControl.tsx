import { useEffect, useRef, useState } from 'react';
import { VolumeHigh, VolumeLow, VolumeMute } from '../ui/icons';
import { usePlayerStore } from '../../stores/playerStore';

/**
 * Output level, as a speaker that opens a vertical slider.
 *
 * `playerStore` has had `volume` since it was written — persisted to
 * localStorage, applied to the audio element on every change — and nothing
 * ever rendered a control for it, so the only level available was the 0.9
 * default. Auditioning next to a running deck is most of what this player is
 * for, and it was fixed at a level chosen by a constant.
 *
 * Vertical rather than inline: it lives in the waveform's corner beside the
 * zoom, where a horizontal slider would either be too short to aim at or
 * would push the clock off the strip.
 */
export function VolumeControl() {
  const volume = usePlayerStore((s) => s.volume);
  const setVolume = usePlayerStore((s) => s.setVolume);
  const [open, setOpen] = useState(false);
  const boxRef = useRef<HTMLSpanElement>(null);
  /** Level to come back to when un-muting. */
  const beforeMute = useRef(volume || 0.9);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!boxRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  const Icon = volume === 0 ? VolumeMute : volume < 0.5 ? VolumeLow : VolumeHigh;

  return (
    <span ref={boxRef} className="relative inline-flex">
      <button
        type="button"
        aria-label={`Volume ${Math.round(volume * 100)}%`}
        title={`Volume ${Math.round(volume * 100)}%`}
        onPointerDown={(e) => e.stopPropagation()}
        onClick={() => setOpen((v) => !v)}
        // Alt-click mutes without opening, which is the gesture for silencing
        // an audition fast when a deck comes in over the top of it.
        onAuxClick={(e) => {
          e.preventDefault();
          if (volume > 0) {
            beforeMute.current = volume;
            setVolume(0);
          } else {
            setVolume(beforeMute.current);
          }
        }}
        className="flex h-6 w-6 items-center justify-center rounded text-gray-400 transition-colors hover:text-keystone-white"
      >
        <Icon size={13} color="currentColor" variant="Linear" />
      </button>

      {open && (
        <span
          onPointerDown={(e) => e.stopPropagation()}
          className="absolute bottom-full right-0 z-50 mb-1 flex h-28 w-8 flex-col items-center justify-center rounded-md border border-gray-700 bg-gray-900 py-2 shadow-xl"
        >
          <input
            type="range"
            min={0}
            max={100}
            value={Math.round(volume * 100)}
            aria-label="Volume"
            onChange={(e) => setVolume(Number(e.target.value) / 100)}
            // A horizontal range rotated upright: `writing-mode: vertical-lr`
            // is the modern way but WebKit's support for styling it is still
            // uneven, and the slider thumb here is a themed pseudo-element.
            style={{
              width: '84px',
              transform: 'rotate(-90deg)',
              transformOrigin: 'center',
            }}
            className="ks-slider"
          />
        </span>
      )}
    </span>
  );
}

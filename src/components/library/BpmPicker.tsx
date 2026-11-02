import { useEffect, useRef, useState } from 'react';
import { TempoValue } from './TrackValues';
import { useSetTrackTempo } from '../../hooks/useLibrary';
import type { TempoDecimals, Track } from '../../types';

/** What a DJ will actually type. Outside this, it is a typo. */
const MIN_BPM = 20;
const MAX_BPM = 300;

/**
 * The BPM readout, clickable, with a tempo field behind it.
 *
 * `set_track_tempo` was the third override in the same state the key and the
 * energy were in: written, registered, reachable from the backend and called
 * by nothing — so the one number a DJ beatmatches to could not be corrected
 * from inside the app.
 *
 * The halve and double buttons are the reason this is a picker rather than a
 * plain field. Octave errors are the common tempo mistake, not arbitrary
 * wrong readings: drum & bass reads at 87 instead of 174, half-time hip-hop
 * at 140 instead of 70. Both are one press here, and typing the corrected
 * number by hand is the slow path for the rarer case.
 *
 * `set_tempo` writes raw and display together and zeroes the octave shift —
 * a typed tempo is already what the user wants to see, so there is nothing
 * left to fold — and stamps `tempoSource: 'user'`, which is what keeps a
 * later re-analysis from reverting the correction.
 */
export function BpmPicker({
  track,
  decimals,
}: {
  track: Track;
  decimals: TempoDecimals;
}) {
  const [open, setOpen] = useState(false);
  const boxRef = useRef<HTMLSpanElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const setTempo = useSetTrackTempo();
  const [draft, setDraft] = useState('');

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

  // A different track is a different tempo; leaving the field up invites a
  // correction aimed at the track that just left.
  useEffect(() => setOpen(false), [track.id]);

  const current = track.tempoDisplay;

  const openWith = () => {
    setDraft(current == null ? '' : String(round(current)));
    setOpen(true);
    // After the field exists.
    requestAnimationFrame(() => inputRef.current?.select());
  };

  const commit = (bpm: number) => {
    if (!Number.isFinite(bpm) || bpm < MIN_BPM || bpm > MAX_BPM) return;
    setTempo.mutate({ id: track.id, bpm: round(bpm) });
    setOpen(false);
  };

  return (
    <span ref={boxRef} className="relative inline-flex">
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="Click to set the tempo by hand"
        onPointerDown={(e) => e.stopPropagation()}
        onDoubleClick={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          if (open) setOpen(false);
          else openWith();
        }}
        className="rounded transition-opacity hover:opacity-80"
      >
        <TempoValue track={track} decimals={decimals} />
      </button>

      {open && (
        <div
          role="dialog"
          aria-label="Set tempo"
          className="absolute left-1/2 top-full z-50 mt-2 w-[196px] -translate-x-1/2 rounded-lg border border-gray-700 bg-gray-900 p-3 shadow-2xl"
        >
          <p className="label mb-2 text-center">Set tempo</p>

          <div className="flex items-center gap-1">
            <Nudge label="−1" onClick={() => bump(draft, current, -1, setDraft)} />
            <input
              ref={inputRef}
              type="text"
              inputMode="decimal"
              value={draft}
              aria-label="BPM"
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  commit(parseFloat(draft));
                }
              }}
              className="mono tnum h-7 min-w-0 flex-1 rounded border border-gray-700 bg-gray-800 px-2 text-center text-xs text-keystone-white outline-none focus:border-keystone-lime"
            />
            <Nudge label="+1" onClick={() => bump(draft, current, 1, setDraft)} />
          </div>

          <div className="mt-1.5 flex gap-1">
            <Nudge
              label="Halve"
              grow
              onClick={() => commit(numeric(draft, current) / 2)}
            />
            <Nudge
              label="Double"
              grow
              onClick={() => commit(numeric(draft, current) * 2)}
            />
          </div>

          <button
            type="button"
            onClick={() => commit(parseFloat(draft))}
            className="mt-1.5 h-7 w-full rounded bg-keystone-lime text-[11px] font-medium text-[color:var(--color-on-accent)] transition-colors hover:bg-[color:var(--color-accent-hover)]"
          >
            Set
          </button>

          <p className="mt-2 text-center text-[11px] text-gray-500">
            Survives re-analysis.
          </p>
        </div>
      )}
    </span>
  );
}

function Nudge({
  label,
  grow = false,
  onClick,
}: {
  label: string;
  grow?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`mono h-7 shrink-0 rounded border border-gray-700 px-2 text-[11px] text-gray-300 transition-colors hover:bg-gray-800 hover:text-keystone-white ${
        grow ? 'flex-1' : ''
      }`}
    >
      {label}
    </button>
  );
}

/** Two decimals is the most the beat grid can justify; more is noise. */
function round(bpm: number): number {
  return Math.round(bpm * 100) / 100;
}

function numeric(draft: string, fallback: number | null): number {
  const parsed = parseFloat(draft);
  return Number.isFinite(parsed) ? parsed : (fallback ?? 0);
}

/** Nudges edit the field rather than committing, so they can be combined. */
function bump(
  draft: string,
  fallback: number | null,
  by: number,
  setDraft: (v: string) => void,
): void {
  setDraft(String(round(numeric(draft, fallback) + by)));
}

import { useEffect, useRef, useState } from 'react';
import { EnergyValue } from '../library/TrackValues';
import { useSetTrackEnergy } from '../../hooks/useLibrary';
import type { Track } from '../../types';

const STEPS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

/**
 * The player's ENERGY readout, clickable, with a 1-10 picker behind it.
 *
 * `set_track_energy` was in the same state the key override was: present in
 * the backend, called by nothing, so the number could not be corrected from
 * inside the app. It matters more here than it does for the key. Keystone's
 * energy scale had its spread fitted on a real corpus, but its *ordering* was
 * never validated — whether one track should outrank another still rests on
 * weights nobody has tested against a human ear. When the engine and the user
 * disagree about energy, the user is the better authority, and until this
 * existed they had no way to say so.
 */
export function EnergyPicker({ track }: { track: Track }) {
  const [open, setOpen] = useState(false);
  const boxRef = useRef<HTMLSpanElement>(null);
  const setEnergy = useSetTrackEnergy();

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

  // A different track is a different number; leaving the picker up invites a
  // correction aimed at the track that just left.
  useEffect(() => setOpen(false), [track.id]);

  const choose = (energy: number) => {
    setEnergy.mutate({ id: track.id, energy });
    setOpen(false);
  };

  return (
    <span ref={boxRef} className="relative inline-flex">
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="Click to set the energy by hand"
        onClick={() => setOpen((v) => !v)}
        className="rounded transition-opacity hover:opacity-80"
      >
        <EnergyValue track={track} />
      </button>

      {open && (
        <div
          role="dialog"
          aria-label="Set energy"
          className="absolute left-1/2 top-full z-50 mt-2 -translate-x-1/2 rounded-lg border border-gray-700 bg-gray-900 p-3 shadow-2xl"
        >
          <p className="label mb-2 text-center">Set energy</p>
          <div className="flex items-end gap-1">
            {STEPS.map((n) => (
              <button
                key={n}
                type="button"
                onClick={() => choose(n)}
                aria-label={`Energy ${n}`}
                aria-pressed={track.energy === n}
                // Each step is as tall as the value it stands for, so the row
                // is the same ramp the table cell draws. Picking a number off
                // a shape you already read is quicker than picking it off ten
                // identical boxes.
                className={`mono flex w-6 shrink-0 flex-col items-center justify-end gap-1 rounded-sm pb-1 pt-1 text-[10px] transition-colors ${
                  track.energy === n
                    ? 'bg-keystone-lime/20 text-keystone-lime'
                    : 'text-gray-400 hover:bg-gray-800 hover:text-keystone-white'
                }`}
              >
                <span
                  style={{ height: `${8 + n * 2.6}px` }}
                  className={`w-1 rounded-sm ${
                    n <= (track.energy ?? 0) ? 'bg-keystone-lime' : 'bg-gray-700'
                  }`}
                />
                {n}
              </button>
            ))}
          </div>
          <p className="mt-2 text-center text-[11px] text-gray-500">
            Overrides the measured energy.
          </p>
        </div>
      )}
    </span>
  );
}

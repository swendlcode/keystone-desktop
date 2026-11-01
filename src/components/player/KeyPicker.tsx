import { useEffect, useRef, useState } from 'react';
import { CamelotWheel } from '../wheel/CamelotWheel';
import { KeyResult } from '../library/KeyResult';
import { useSetTrackKey } from '../../hooks/useLibrary';
import { fromCamelot, parseCamelot } from '../../utils/camelot';
import type { KeyNotation, Track } from '../../types';

/** Nothing is on the wheel in this popover, so every segment reads as pickable. */
const NO_COUNTS = new Map<string, number>();

/**
 * The player's KEY readout, clickable, with the wheel as its picker.
 *
 * `set_track_key` has been in the backend since it was written and nothing
 * called it, so a key the engine got wrong was unfixable from inside the app —
 * the one number on this bar a DJ is most likely to want the last word on was
 * the one number that was read-only. The backend stamps `keySource: 'user'` on
 * what is chosen here, which is what keeps a later re-analysis from quietly
 * reverting the correction.
 *
 * The picker is the same `CamelotWheel` the sidebar draws, controlled with the
 * track's own key as its selection. A separate grid of 24 buttons would be
 * easier to write and would mean learning a second arrangement of the same
 * 24 keys, which is the thing the wheel exists to avoid.
 */
export function KeyPicker({
  track,
  notation,
  pad,
}: {
  track: Track;
  notation: KeyNotation;
  pad: boolean;
}) {
  const [open, setOpen] = useState(false);
  const boxRef = useRef<HTMLSpanElement>(null);
  const setKey = useSetTrackKey();

  // Close on anything that is not this popover: a picker that survives a click
  // on the table is a picker the user has to come back and dismiss.
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

  // A different track in the player is a different key; leaving the popover up
  // would invite a correction aimed at the track that just left.
  useEffect(() => setOpen(false), [track.id]);

  const choose = (code: string) => {
    const key = parseCamelot(code);
    if (!key) return;
    const { root, mode } = fromCamelot(key);
    setKey.mutate({ id: track.id, keyRoot: root, keyMode: mode });
    setOpen(false);
  };

  return (
    <span ref={boxRef} className="relative inline-flex">
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="Click to set the key by hand"
        onClick={() => setOpen((v) => !v)}
        className="rounded transition-opacity hover:opacity-80"
      >
        <KeyResult track={track} notation={notation} pad={pad} />
      </button>

      {open && (
        <div
          role="dialog"
          aria-label="Set key"
          className="absolute left-1/2 top-full z-50 mt-2 w-[248px] -translate-x-1/2 rounded-lg border border-gray-700 bg-gray-900 p-3 shadow-2xl"
        >
          <p className="label mb-2 text-center">Set key</p>
          <CamelotWheel
            selected={track.camelot}
            playing={null}
            counts={NO_COUNTS}
            includeCompatible={false}
            onSelect={choose}
          />
          <p className="mt-2 text-center text-[11px] text-gray-500">
            Overrides the detected key. Survives re-analysis.
          </p>
        </div>
      )}
    </span>
  );
}

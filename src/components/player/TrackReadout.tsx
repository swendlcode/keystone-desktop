import type { ReactNode } from 'react';
import { Keyboard } from '../ui/icons';
import { CueStepper } from './CueStepper';
import { EnergyPicker } from '../library/EnergyPicker';
import { KeyPicker } from './KeyPicker';
import { TempoValue } from '../library/TrackValues';
import { useUiStore } from '../../stores/uiStore';
import type { Settings, Track } from '../../types';

interface TrackReadoutProps {
  track: Track;
  settings: Settings | null;
  onSeek: (seconds: number) => void;
}

/**
 * `KEY 12A · ENERGY 6 · BPM 130 | CUE POINTS < > ADD CUE | VIRTUAL PIANO`.
 *
 * The key, energy and tempo cells are the same components the library table
 * uses, not look-alikes. That is the only way an uncertain key reads as
 * uncertain in both places: a second rendering of the same value is a second
 * chance to forget the confidence.
 *
 * Three groups with rules between them, not seven items in a row. The readout
 * is three different kinds of thing — what the track *is*, what you can do to
 * its cues, and a panel toggle — and run together at an even gap they read as
 * one undifferentiated strip of words, which is what made the bar feel
 * crowded even before it ran out of room.
 */
export function TrackReadout({ track, settings, onSeek }: TrackReadoutProps) {
  const pianoOpen = useUiStore((s) => s.pianoOpen);
  const togglePiano = useUiStore((s) => s.togglePiano);

  return (
    <span className="flex shrink-0 items-center gap-5">
      <Readout label="Key">
        <KeyPicker
          track={track}
          notation={settings?.keyNotation ?? 'camelot'}
          pad={settings?.padCamelotNumbers ?? false}
        />
      </Readout>

      <Readout label="Energy">
        <EnergyPicker track={track} />
      </Readout>

      <Readout label="BPM">
        <TempoValue track={track} decimals={settings?.tempoDecimals ?? 0} />
      </Readout>

      <Divider />

      <CueStepper trackId={track.id} onSeek={onSeek} />

      <Divider />

      <button
        type="button"
        onClick={togglePiano}
        aria-pressed={pianoOpen}
        title="Show the detected key's scale"
        className={`label flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors ${
          pianoOpen
            ? 'bg-keystone-lime/15 text-keystone-lime'
            : 'hover:bg-gray-800 hover:text-keystone-white'
        }`}
      >
        <Keyboard size={12} color="currentColor" variant="Linear" />
        Virtual piano
      </button>
    </span>
  );
}

function Readout({ label, children }: { label: string; children: ReactNode }) {
  return (
    <span className="flex items-center gap-2">
      <span className="label">{label}</span>
      {children}
    </span>
  );
}

/** A hairline between the three groups. */
function Divider() {
  return <span aria-hidden className="h-4 w-px shrink-0 bg-gray-700" />;
}

import { PlaybackControls } from './PlaybackControls';
import { TrackReadout } from './TrackReadout';
import { VirtualPiano } from './VirtualPiano';
import { WaveformView } from './WaveformView';
import { usePlayer } from '../../hooks/usePlayer';
import { useEffectiveSettings } from '../../hooks/useSettings';
import { usePlayerStore } from '../../stores/playerStore';
import { useUiStore } from '../../stores/uiStore';
import { trackLabel } from '../../utils/formatters';

/**
 * The player across the top of the main window.
 *
 * Mounts `usePlayer`, which owns the single audio element, so this component
 * has to stay mounted for the life of the window rather than per view —
 * App renders it outside the page switch for that reason.
 *
 * Layout order is SPEC.md §1's: transport, then the full-width waveform with
 * its eight markers, then `ARTIST - TITLE`, then the readout row. The waveform
 * gets its own row rather than sharing one with the transport because
 * "full-width" is what makes a cue marker draggable to a useful precision.
 */
export function PlayerBar() {
  const { seek } = usePlayer();
  const track = usePlayerStore((s) => s.currentTrack);
  const duration = usePlayerStore((s) => s.duration);
  const pianoOpen = useUiStore((s) => s.pianoOpen);
  const settings = useEffectiveSettings();

  // The element's own duration once it has metadata, the indexed value until
  // then: the table has a duration long before the file is opened.
  const totalS = duration || (track?.durationMs ?? 0) / 1000;

  return (
    <div className="flex shrink-0 flex-col gap-2 border-b border-gray-700 bg-gray-900 px-3 py-2.5">
      <PlaybackControls totalS={totalS} />

      {track ? (
        <WaveformView track={track} onSeek={seek} />
      ) : (
        <div className="relative h-[72px] w-full rounded-md bg-gray-800">
          <span className="absolute inset-x-0 top-1/2 h-px bg-gray-700" />
        </div>
      )}

      {/* `flex-wrap`: on a narrow window the readout drops to its own line
          instead of squeezing the title into an ellipsis after four words.
          The title is the one thing here that cannot be guessed from a
          fragment. */}
      <div className="flex min-w-0 flex-wrap items-center gap-x-6 gap-y-2">
        <span className="min-w-0 flex-1 truncate text-xs font-semibold uppercase tracking-wide text-keystone-white">
          {track ? trackLabel(track) : 'No track'}
        </span>
        {track && (
          <TrackReadout track={track} settings={settings} onSeek={seek} />
        )}
      </div>

      {pianoOpen && track && (
        <div className="border-t border-gray-700">
          <VirtualPiano
            keyRoot={track.keyRoot}
            keyMode={track.keyMode}
            notation={settings?.keyNotation ?? 'camelot'}
          />
        </div>
      )}
    </div>
  );
}

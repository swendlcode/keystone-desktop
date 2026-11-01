import { PlaybackControls, WaveformMeta } from './PlaybackControls';
import { TrackReadout } from './TrackReadout';
import { VirtualPiano } from './VirtualPiano';
import { WaveformView } from './WaveformView';
import { PlaybackError } from './PlaybackError';
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
 * The transport sits BESIDE the waveform rather than above it, and the clock
 * and zoom are overlaid in the waveform's bottom-right corner. Three stacked
 * rows — transport, waveform, readout — spent two of them on a handful of
 * controls and left the waveform, the only element here that benefits from
 * height, with the least of it. Beside the wave costs about 90px of width,
 * which a cue marker does not miss, and gives the row back.
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
    // `relative`: the playback error is an overlay rather than a row. As a
    // row it changed the bar's height when it appeared, which shoved the
    // whole table down a line at the exact moment something had already gone
    // wrong.
    <div className="flex shrink-0 flex-col gap-2 border-b border-gray-700 bg-gray-900 px-3 py-2.5">
      <div className="flex items-center gap-3">
        <PlaybackControls />

        <div className="relative min-w-0 flex-1">
          {track ? (
            <WaveformView track={track} onSeek={seek} />
          ) : (
            <div className="relative h-[84px] w-full rounded-md bg-gray-800">
              <span className="absolute inset-x-0 top-1/2 h-px bg-gray-700" />
            </div>
          )}
          <WaveformMeta totalS={totalS} />
          {/* Inside the waveform, bottom-left — the one area with nothing in
              it. Pinned to the bar's bottom edge it covered the title and
              the readout, which is worse than the layout shift it was
              avoiding. */}
          <PlaybackError />
        </div>
      </div>

      {/* Two rows, always. The title shared a line with the readout and lost
          every time — the readout is a fixed set of controls that cannot
          shrink, so the title absorbed the whole shortfall and became an
          ellipsis after four words. A track's name is the one thing on this
          bar that cannot be guessed from a fragment, and an artist, a title,
          a mix name and a label do not fit in what is left over beside six
          controls. It gets the row. */}
      <span className="block w-full text-sm font-semibold uppercase tracking-wide text-keystone-white">
        {track ? trackLabel(track) : 'No track'}
      </span>

      {track && (
        <div className="flex min-w-0 flex-wrap items-center gap-x-6 gap-y-2">
          <TrackReadout track={track} settings={settings} onSeek={seek} />
        </div>
      )}

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

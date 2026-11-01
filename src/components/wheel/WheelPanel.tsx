import { CamelotWheel } from './CamelotWheel';
import { useWheelCounts } from '../../hooks/useLibrary';
import { useLibraryStore } from '../../stores/libraryStore';
import { usePlayerStore } from '../../stores/playerStore';
import { formatCount } from '../../utils/formatters';

/**
 * The wheel plus the two facts that belong next to it.
 *
 * This is the only wheel file that reads a store, so `CamelotWheel` stays a
 * controlled component. The split matters because the wheel is also the
 * obvious thing to reuse in a song-info panel, where the selection is not the
 * library filter.
 */
export function WheelPanel() {
  const { data } = useWheelCounts();
  const selected = useLibraryStore((s) => s.filters.camelot);
  const includeCompatible = useLibraryStore(
    (s) => s.filters.camelotIncludeCompatible,
  );
  const selectCamelot = useLibraryStore((s) => s.selectCamelot);
  const setIncludeCompatible = useLibraryStore(
    (s) => s.setCamelotIncludeCompatible,
  );
  const playing = usePlayerStore((s) => s.currentTrack?.camelot ?? null);

  const counts = new Map(data?.counts ?? []);

  return (
    <div className="px-3 pb-3 pt-3">
      {/* Fixed height, and the button is hidden rather than unmounted.
          Neighbours only applies to a selection, so it used to appear the
          moment one was made — which made this row taller and shoved the
          wheel, ADD TRACKS and the whole collections list down a few pixels
          on the very click that was meant to be aimed at the wheel. Reserving
          the space costs nothing and the column stops jumping. */}
      <div className="mb-2 flex h-5 items-center justify-between">
        <span className="label">Key</span>
        <button
          type="button"
          onClick={() => setIncludeCompatible(!includeCompatible)}
          aria-pressed={includeCompatible}
          aria-hidden={!selected}
          tabIndex={selected ? 0 : -1}
          title={
            includeCompatible
              ? 'Also showing the three compatible keys'
              : 'Showing this key only'
          }
          className={`label rounded px-1.5 py-0.5 transition-colors ${
            selected ? '' : 'invisible'
          } ${
            includeCompatible
              ? 'bg-keystone-lime/15 text-keystone-lime'
              : 'hover:bg-gray-800 hover:text-keystone-white'
          }`}
        >
          Neighbours
        </button>
      </div>

      <CamelotWheel
        selected={selected}
        playing={playing}
        counts={counts}
        includeCompatible={includeCompatible}
        onSelect={selectCamelot}
      />

      {/* One reserved row under the wheel, for the same no-jump reason as the
          header above it. Clearing the filter is a real button here rather
          than a word in the hole of the wheel: it is the way out of the app's
          stickiest filter, and in the centre it was small, unlabelled as a
          control, and sat on top of the key it was about to discard. The
          unkeyed count takes the row when there is nothing to clear — it is a
          library-wide figure and does not describe a filtered view anyway. */}
      <div className="mt-2 flex h-7 items-center justify-center">
        {selected ? (
          <button
            type="button"
            onClick={() => selectCamelot(selected)}
            className="label w-full rounded-md border border-gray-600 py-1.5 text-center transition-colors hover:border-keystone-lime/60 hover:bg-gray-800 hover:text-keystone-white"
          >
            Clear filter
          </button>
        ) : (
          data &&
          data.unkeyed > 0 && (
            <p className="mono text-center text-[12px] text-gray-500">
              {formatCount(data.unkeyed)} unkeyed
            </p>
          )
        )}
      </div>
    </div>
  );
}

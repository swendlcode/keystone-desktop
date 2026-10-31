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
      <div className="mb-2 flex items-center justify-between">
        <span className="label">Key</span>
        {selected && (
          <button
            type="button"
            onClick={() => setIncludeCompatible(!includeCompatible)}
            aria-pressed={includeCompatible}
            title={
              includeCompatible
                ? 'Also showing the three compatible keys'
                : 'Showing this key only'
            }
            className={`label rounded px-1.5 py-0.5 transition-colors ${
              includeCompatible
                ? 'bg-keystone-lime/15 text-keystone-lime'
                : 'hover:bg-gray-800 hover:text-keystone-white'
            }`}
          >
            Neighbours
          </button>
        )}
      </div>

      <CamelotWheel
        selected={selected}
        playing={playing}
        counts={counts}
        includeCompatible={includeCompatible}
        onSelect={selectCamelot}
        // `selectCamelot(code)` with the code already selected is the store's
        // own clear, so Clear passes the selection back rather than null.
        onClear={() => selected && selectCamelot(selected)}
      />

      {data && data.unkeyed > 0 && (
        <p className="mono mt-2 text-center text-[11px] text-gray-500">
          {formatCount(data.unkeyed)} unkeyed
        </p>
      )}
    </div>
  );
}

import { useEffect, useRef, useState } from 'react';
import { CloseCircle, SearchNormal } from '../ui/icons';
import { Input } from '../ui/Input';
import { useCollections } from '../../hooks/useCollections';
import { useLibraryStore } from '../../stores/libraryStore';
import type { TrackFilters } from '../../types';
import { formatCount } from '../../utils/formatters';

/** Typing is debounced into the store; the store keys the query. */
const DEBOUNCE_MS = 180;

/**
 * Search, the track count, and whatever is currently narrowing the view.
 *
 * The field keeps a local draft and pushes it on a timer, then adopts the
 * store's value back only when something *else* wrote it — a chip's clear
 * button, or the wheel resetting the query. `lastPushed` is how an echo of
 * this component's own write is told apart from someone else's, which is the
 * difference between a debounced field and an infinite loop.
 */
export function LibraryToolbar({ total }: { total: number }) {
  const query = useLibraryStore((s) => s.filters.query);
  const setQuery = useLibraryStore((s) => s.setQuery);
  const [draft, setDraft] = useState(query);
  const lastPushed = useRef(query);

  useEffect(() => {
    if (draft === lastPushed.current) return;
    const t = setTimeout(() => {
      lastPushed.current = draft;
      setQuery(draft);
    }, DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [draft, setQuery]);

  useEffect(() => {
    if (query !== lastPushed.current) {
      lastPushed.current = query;
      setDraft(query);
    }
  }, [query]);

  return (
    <div className="flex shrink-0 flex-col gap-2 border-b border-gray-700 bg-gray-900 px-3 py-2">
      <div className="flex items-center gap-3">
        <Input
          value={draft}
          placeholder="Search"
          onChange={(e) => setDraft(e.target.value)}
          className="h-8 max-w-sm flex-1"
          leading={
            <SearchNormal size={14} color="currentColor" variant="Linear" />
          }
          trailing={
            draft ? (
              <button
                type="button"
                aria-label="Clear search"
                onClick={() => setDraft('')}
                className="text-gray-400 hover:text-keystone-white"
              >
                <CloseCircle size={14} color="currentColor" variant="Linear" />
              </button>
            ) : undefined
          }
        />
        <span className="mono tnum label shrink-0">
          {formatCount(total)} tracks
        </span>
        <FilterChips />
      </div>
    </div>
  );
}

/**
 * One chip per active narrowing, each clearing only itself.
 *
 * The collection is a chip too even though the sidebar row is already lit:
 * `resetFilters` deliberately keeps the collection, so without a chip there is
 * no way to leave a collection from the table side of the window.
 */
function FilterChips() {
  const filters = useLibraryStore((s) => s.filters);
  const selectCamelot = useLibraryStore((s) => s.selectCamelot);
  const selectCollection = useLibraryStore((s) => s.selectCollection);
  const patchFilters = useLibraryStore((s) => s.patchFilters);
  const resetFilters = useLibraryStore((s) => s.resetFilters);
  const { data: collections = [] } = useCollections();

  const collection = collections.find((c) => c.id === filters.collectionId);
  const extra = countExtraFilters(filters);

  if (!filters.camelot && !collection && !filters.improvableOnly && extra === 0) {
    return null;
  }

  return (
    <span className="flex min-w-0 items-center gap-1.5 overflow-hidden">
      {filters.camelot && (
        <Chip
          label={
            filters.camelotIncludeCompatible
              ? `${filters.camelot} +3`
              : filters.camelot
          }
          onClear={() => selectCamelot(filters.camelot)}
        />
      )}
      {collection && (
        <Chip label={collection.name} onClear={() => selectCollection(null)} />
      )}
      {filters.improvableOnly && (
        <Chip
          label="Improvable"
          onClear={() => patchFilters({ improvableOnly: false })}
        />
      )}
      {extra > 0 && <Chip label={`+${extra}`} onClear={resetFilters} />}
      <button
        type="button"
        onClick={resetFilters}
        className="label shrink-0 rounded px-1.5 py-0.5 hover:bg-gray-800 hover:text-keystone-white"
      >
        Clear all
      </button>
    </span>
  );
}

/** The narrowings that have no chip of their own, counted for the `+N` chip. */
function countExtraFilters(filters: TrackFilters): number {
  let n = 0;
  if (filters.energyMin != null || filters.energyMax != null) n += 1;
  if (filters.tempoMin != null || filters.tempoMax != null) n += 1;
  if (filters.genres.length > 0) n += 1;
  if (filters.ratingMin != null) n += 1;
  if (filters.analyzed != null) n += 1;
  if (filters.pathPrefix != null) n += 1;
  return n;
}

function Chip({ label, onClear }: { label: string; onClear: () => void }) {
  return (
    <span className="badge shrink-0 bg-gray-700 text-keystone-white">
      {label}
      <button
        type="button"
        aria-label={`Clear ${label}`}
        onClick={onClear}
        className="text-gray-400 hover:text-keystone-white"
      >
        <CloseCircle size={11} color="currentColor" variant="Linear" />
      </button>
    </span>
  );
}

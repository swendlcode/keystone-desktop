import { create } from 'zustand';
import {
  DEFAULT_DIRECTION,
  DEFAULT_FILTERS,
  DEFAULT_SORT,
  type ScanProgress,
  type SortDirection,
  type TrackFilters,
  type TrackSort,
} from '../types';

/**
 * Client state for the library view: what is filtered, sorted and selected.
 *
 * The *rows* are react-query's business, keyed on this filter object. Putting
 * them here as well is the mistake that makes a library view go stale after a
 * re-tag, because two caches then have to be invalidated by hand.
 */
interface LibraryStore {
  filters: TrackFilters;
  sort: TrackSort;
  direction: SortDirection;
  /** Row ids, for the context menu and the bulk tag actions. */
  selectedIds: Set<string>;
  /** Anchor for shift-click range selection. */
  lastClickedId: string | null;
  /** Live walk progress, pushed from the scan-progress event. */
  scanProgress: ScanProgress | null;

  setQuery: (query: string) => void;
  /** Clicking a wheel segment. Clicking the selected one again clears it. */
  selectCamelot: (code: string | null) => void;
  setCamelotIncludeCompatible: (include: boolean) => void;
  selectCollection: (collectionId: string | null) => void;
  patchFilters: (patch: Partial<TrackFilters>) => void;
  /** Clicking a column header: same column flips the direction. */
  toggleSort: (field: TrackSort) => void;
  resetFilters: () => void;

  select: (id: string) => void;
  toggleSelect: (id: string) => void;
  selectRange: (ids: string[]) => void;
  clearSelection: () => void;

  setScanProgress: (progress: ScanProgress | null) => void;
}

export const useLibraryStore = create<LibraryStore>((set) => ({
  filters: DEFAULT_FILTERS,
  sort: DEFAULT_SORT,
  direction: DEFAULT_DIRECTION,
  selectedIds: new Set(),
  lastClickedId: null,
  scanProgress: null,

  setQuery: (query) =>
    set((s) => ({ filters: { ...s.filters, query } })),

  selectCamelot: (code) =>
    set((s) => ({
      filters: {
        ...s.filters,
        camelot: s.filters.camelot === code ? null : code,
      },
    })),

  setCamelotIncludeCompatible: (camelotIncludeCompatible) =>
    set((s) => ({ filters: { ...s.filters, camelotIncludeCompatible } })),

  selectCollection: (collectionId) =>
    set((s) => ({ filters: { ...s.filters, collectionId } })),

  patchFilters: (patch) =>
    set((s) => ({ filters: { ...s.filters, ...patch } })),

  toggleSort: (field) =>
    set((s) =>
      s.sort === field
        ? { direction: s.direction === 'asc' ? 'desc' : 'asc' }
        : { sort: field, direction: 'asc' },
    ),

  /**
   * Clear all. Deliberately keeps the collection: the sidebar row stays lit, so
   * dumping the user into the whole library while it is highlighted is a lie
   * about where they are. Deselecting the collection is its own click.
   */
  resetFilters: () =>
    set((s) => ({
      filters: { ...DEFAULT_FILTERS, collectionId: s.filters.collectionId },
    })),

  select: (id) => set({ selectedIds: new Set([id]), lastClickedId: id }),

  toggleSelect: (id) =>
    set((s) => {
      const next = new Set(s.selectedIds);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return { selectedIds: next, lastClickedId: id };
    }),

  selectRange: (ids) => set({ selectedIds: new Set(ids) }),

  clearSelection: () => set({ selectedIds: new Set(), lastClickedId: null }),

  setScanProgress: (scanProgress) => set({ scanProgress }),
}));

/** Ids as an array, in no particular order — for passing to a command. */
export function selectedTrackIds(): string[] {
  return [...useLibraryStore.getState().selectedIds];
}

import { PAGE_SIZE, useTracks } from '../../hooks/useLibrary';
import type { Track } from '../../types';

export interface TrackWindow {
  /** Total matching rows — the virtualizer's count and the `N TRACKS` readout. */
  total: number;
  /** The row at a library-wide index, or null while its page is in flight. */
  trackAt: (index: number) => Track | null;
  /** The loaded rows in order, for range selection and the play queue. */
  loaded: Track[];
  isLoading: boolean;
}

/**
 * Two pages around the scroll position, addressed by library-wide index.
 *
 * The table is virtualized over `total` rather than over an array, so the rows
 * it has are a window and the rows it does not have render as placeholders.
 * Two pages rather than one because a 400-row buffer at 200 per page means
 * the next page is almost always already there when the user crosses a
 * boundary, and `placeholderData` in `useTracks` keeps the old one on screen
 * while it is not.
 */
export function useTrackWindow(page: number): TrackWindow {
  const current = useTracks(page);
  const ahead = useTracks(page + 1);

  const pages = new Map<number, Track[]>();
  if (current.data) pages.set(page, current.data.tracks);
  if (ahead.data) pages.set(page + 1, ahead.data.tracks);

  const trackAt = (index: number): Track | null => {
    const rows = pages.get(Math.floor(index / PAGE_SIZE));
    return rows?.[index % PAGE_SIZE] ?? null;
  };

  const loaded = [...(pages.get(page) ?? []), ...(pages.get(page + 1) ?? [])];

  return {
    total: current.data?.total ?? 0,
    trackAt,
    loaded,
    isLoading: current.isLoading,
  };
}

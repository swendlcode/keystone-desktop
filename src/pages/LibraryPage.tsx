import { useCallback, useState } from 'react';
import { LibraryToolbar } from '../components/library/LibraryToolbar';
import { TrackTable } from '../components/library/TrackTable';
import { useTrackWindow } from '../components/library/useTrackWindow';

/**
 * SPEC.md §1 — the library table, under the player and beside the wheel.
 *
 * Takes no props: the filters, sort and selection live in `libraryStore`, the
 * visible column set in the Display Options setting, and the rows in
 * react-query. The page owns only the scroll position's page index, because
 * both the toolbar's count and the table's window are read from the same pair
 * of page queries and a second copy would be a second fetch.
 */
export function LibraryPage() {
  const [page, setPage] = useState(0);
  const window = useTrackWindow(page);

  // Stable, so the table's memoised rows are not invalidated on every scroll
  // frame by a fresh callback identity.
  const onPageChange = useCallback((next: number) => setPage(next), []);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <LibraryToolbar total={window.total} />
      <TrackTable window={window} onPageChange={onPageChange} />
    </div>
  );
}

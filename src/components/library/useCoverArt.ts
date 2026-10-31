import { useQuery } from '@tanstack/react-query';
import { coverArtUrl, keystoneService } from '../../services/keystoneService';
import type { Track } from '../../types';

/**
 * Embedded cover art for one row, as a data URL.
 *
 * Only ever called from a mounted row, so the virtualizer is the fetch budget:
 * a 20,000-track library asks for the forty thumbnails on screen, not twenty
 * thousand. `enabled` on `hasCoverArt` keeps the rest from being asked for at
 * all, which is most of a DJ library.
 *
 * Art never changes for a file unless the user replaces it, so this never
 * re-validates; the tag editor invalidates the key when it writes.
 */
export function useCoverArt(track: Track) {
  const query = useQuery({
    queryKey: coverArtKeys.byTrack(track.id),
    queryFn: () => keystoneService.getCoverArt(track.id),
    enabled: track.hasCoverArt,
    staleTime: Infinity,
    // Scrolling back up a long table should not re-decode what was on screen
    // a second ago, but holding every thumbnail of a 20,000-row library would
    // be hundreds of megabytes of base64. Five minutes is the compromise.
    gcTime: 5 * 60_000,
  });

  return query.data ? coverArtUrl(query.data) : null;
}

export const coverArtKeys = {
  all: ['cover-art'] as const,
  byTrack: (trackId: string) => ['cover-art', trackId] as const,
};

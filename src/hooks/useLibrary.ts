import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../services/keystoneService';
import { useLibraryStore } from '../stores/libraryStore';
import { forgetTracks } from '../stores/playerStore';
import {
  KeystoneEvent,
  type KeyMode,
  type ScanProgressEvent,
  type TrackPage,
  type TrackQuery,
  type TrackUpdatedEvent,
} from '../types';

export const PAGE_SIZE = 200;

export const trackKeys = {
  all: ['tracks'] as const,
  page: (query: TrackQuery) => [...trackKeys.all, 'page', query] as const,
  byId: (id: string) => [...trackKeys.all, 'detail', id] as const,
  wheel: (filters: unknown) => [...trackKeys.all, 'wheel', filters] as const,
  genres: [...['tracks'], 'genres'] as const,
};

export const folderKeys = {
  all: ['watched-folders'] as const,
};

/** One page of the library table under the current filters and sort. */
export function useTracks(page = 0, pageSize = PAGE_SIZE) {
  const filters = useLibraryStore((s) => s.filters);
  const sort = useLibraryStore((s) => s.sort);
  const direction = useLibraryStore((s) => s.direction);

  const query: TrackQuery = {
    filters,
    sort,
    direction,
    limit: pageSize,
    offset: page * pageSize,
  };

  return useQuery({
    queryKey: trackKeys.page(query),
    queryFn: () => keystoneService.searchTracks(query),
    // Keep the previous page on screen while the next one loads, so scrolling
    // a 20,000-row table does not blank between fetches.
    placeholderData: (prev) => prev,
  });
}

export function useTrack(id: string | null) {
  return useQuery({
    queryKey: id ? trackKeys.byId(id) : [...trackKeys.all, 'detail', 'none'],
    queryFn: () => (id ? keystoneService.getTrack(id) : null),
    enabled: Boolean(id),
  });
}

/**
 * Per-segment counts for the wheel.
 *
 * Deliberately keyed on the filters *without* the Camelot selection: the counts
 * answer "how many tracks would this segment give me", so folding the current
 * segment into them would make every other segment read zero.
 */
export function useWheelCounts() {
  const filters = useLibraryStore((s) => s.filters);
  const unscoped = { ...filters, camelot: null };
  return useQuery({
    queryKey: trackKeys.wheel(unscoped),
    queryFn: () => keystoneService.getWheelCounts(unscoped),
    staleTime: 30_000,
  });
}

export function useGenres() {
  return useQuery({
    queryKey: trackKeys.genres,
    queryFn: () => keystoneService.getGenres(),
    staleTime: 5 * 60_000,
  });
}

export function useWatchedFolders() {
  return useQuery({
    queryKey: folderKeys.all,
    queryFn: () => keystoneService.getWatchedFolders(),
  });
}

export function useAddFolder() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => keystoneService.addWatchedFolder(path),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: folderKeys.all });
      qc.invalidateQueries({ queryKey: trackKeys.all });
    },
  });
}

/**
 * Stop watching a folder. Keyed by path, not by id.
 *
 * The backend cascades by path prefix — every track under the folder leaves the
 * library with it — so the path is the argument it actually needs, and handing
 * it an id would delete nothing while reporting success.
 */
export function useRemoveFolder() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => keystoneService.removeWatchedFolder(path),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: folderKeys.all });
      qc.invalidateQueries({ queryKey: trackKeys.all });
    },
  });
}

export function useSetRating() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, rating }: { id: string; rating: number }) =>
      keystoneService.setRating(id, rating),
    onSuccess: (track) => {
      qc.setQueryData(trackKeys.byId(track.id), track);
      patchTrackInPages(qc, track);
    },
  });
}

/**
 * Override the detected key by hand.
 *
 * The command has existed since the backend was written and nothing called
 * it, so a wrong letter was unfixable from inside the app. The backend stamps
 * `keySource: 'user'`, which is what stops a later re-analysis quietly undoing
 * the correction.
 */
export function useSetTrackKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      keyRoot,
      keyMode,
    }: {
      id: string;
      keyRoot: number;
      keyMode: KeyMode;
    }) => keystoneService.setTrackKey(id, keyRoot, keyMode),
    onSuccess: (track) => {
      qc.setQueryData(trackKeys.byId(track.id), track);
      patchTrackInPages(qc, track);
      // The wheel's per-segment counts move with it. Keyed by the filters it
      // was built for, so invalidate the prefix rather than one entry.
      qc.invalidateQueries({ queryKey: [...trackKeys.all, 'wheel'] });
    },
  });
}

/**
 * Override the measured energy by hand.
 *
 * Same situation the key was in: `set_track_energy` existed and nothing
 * called it. Energy matters more than most for being correctable, because the
 * engine's own ordering is still unproven — the scale's spread was fitted on
 * a real corpus but whether track A should outrank track B was never
 * validated, so the user's ear is the better authority on a disagreement.
 */
export function useSetTrackEnergy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, energy }: { id: string; energy: number }) =>
      keystoneService.setTrackEnergy(id, energy),
    onSuccess: (track) => {
      qc.setQueryData(trackKeys.byId(track.id), track);
      patchTrackInPages(qc, track);
    },
  });
}

export function useRemoveTracks() {
  const qc = useQueryClient();
  const clearSelection = useLibraryStore((s) => s.clearSelection);
  return useMutation({
    mutationFn: (ids: string[]) =>
      keystoneService.removeTracks(ids).then(() => ids),
    onSuccess: (ids) => {
      clearSelection();
      // The transport is not part of the library's cache, so invalidating
      // the rows leaves a removed track playing over an empty table.
      forgetTracks(ids);
      qc.invalidateQueries({ queryKey: trackKeys.all });
    },
  });
}

/**
 * Library-wide event wiring. Mounted once, from App.
 *
 * `track-updated` carries the whole row, so a landed analysis patches the
 * cached pages in place. Invalidating instead would refetch every mounted page
 * once per track and make a 5,000-track run unusable while it ran.
 */
export function useLibrarySync() {
  const qc = useQueryClient();
  const setScanProgress = useLibraryStore((s) => s.setScanProgress);

  useEffect(() => {
    const unsubs: Array<Promise<() => void>> = [];

    unsubs.push(
      listen<ScanProgressEvent>(KeystoneEvent.scanProgress, (e) => {
        setScanProgress(e.payload.progress);
        if (!e.payload.progress.isScanning) {
          qc.invalidateQueries({ queryKey: trackKeys.all });
          qc.invalidateQueries({ queryKey: folderKeys.all });
        }
      }),
    );

    unsubs.push(
      listen<TrackUpdatedEvent>(KeystoneEvent.trackUpdated, (e) => {
        const track = e.payload.track;
        qc.setQueryData(trackKeys.byId(track.id), track);
        patchTrackInPages(qc, track);
      }),
    );

    unsubs.push(
      listen(KeystoneEvent.reconcileComplete, () => {
        qc.invalidateQueries({ queryKey: trackKeys.all });
        qc.invalidateQueries({ queryKey: folderKeys.all });
      }),
    );

    return () => {
      Promise.all(unsubs).then((fns) => fns.forEach((fn) => fn()));
    };
  }, [qc, setScanProgress]);
}

/** Replace one row inside every cached page that holds it. */
function patchTrackInPages(
  qc: ReturnType<typeof useQueryClient>,
  track: import('../types').Track,
): void {
  qc.setQueriesData<TrackPage>(
    { queryKey: [...trackKeys.all, 'page'] },
    (old) => {
      if (!old) return old;
      const i = old.tracks.findIndex((t) => t.id === track.id);
      if (i === -1) return old;
      const tracks = [...old.tracks];
      tracks[i] = track;
      return { ...old, tracks };
    },
  );
}

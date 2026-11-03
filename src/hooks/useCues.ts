import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../services/keystoneService';
import type { CueEdit, CuePoint } from '../types';

export const cueKeys = {
  all: ['cues'] as const,
  byTrack: (trackId: string) => [...cueKeys.all, trackId] as const,
  counts: (trackIds: string[]) => [...cueKeys.all, 'counts', trackIds] as const,
};

/** The eight markers for a track, ascending by slot. */
export function useCuePoints(trackId: string | null) {
  return useQuery({
    queryKey: trackId ? cueKeys.byTrack(trackId) : [...cueKeys.all, 'none'],
    queryFn: () => (trackId ? keystoneService.getCuePoints(trackId) : []),
    enabled: Boolean(trackId),
  });
}

/**
 * How many of the eight slots each track has filled, for the table's CUES
 * column.
 *
 * One query for the whole visible page rather than one per row: the table is
 * virtualized over tens of thousands of rows and a per-row query would be a
 * request per scroll frame. `getCueCounts` was written for this and then
 * called by nothing, so the column read `--` on every row while the player
 * bar showed eight markers for the same track — which reads as "no cues
 * anywhere" rather than "not loaded".
 *
 * Returned as a Map so the table can look a row up without scanning. Rows
 * with no cues are absent from the backend's reply, so a miss is a real zero
 * rather than a gap, and `keepPreviousData` stops the column blanking on
 * every page boundary while the next request is in flight.
 */
export function useCueCounts(trackIds: string[]) {
  return useQuery({
    queryKey: cueKeys.counts(trackIds),
    queryFn: async () => {
      if (trackIds.length === 0) return new Map<string, number>();
      const pairs = await keystoneService.getCueCounts(trackIds);
      return new Map(pairs);
    },
    enabled: trackIds.length > 0,
    placeholderData: (previous) => previous,
  });
}

/**
 * Add or replace a marker.
 *
 * Writes `source: 'user'` on the backend, which is what stops a later
 * re-analysis moving a cue the user dragged.
 */
export function useSetCuePoint() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (edit: CueEdit) => keystoneService.setCuePoint(edit),
    onSuccess: (cue) => patchCue(qc, cue),
  });
}

/** Dragging a marker. Fired on drop, not on every pointer move. */
export function useMoveCuePoint() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, positionMs }: { id: string; positionMs: number }) =>
      keystoneService.moveCuePoint(id, positionMs),
    onSuccess: (cue) => patchCue(qc, cue),
  });
}

export function useDeleteCuePoint() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, trackId }: { id: string; trackId: string }) =>
      keystoneService.deleteCuePoint(id).then(() => trackId),
    onSuccess: (trackId) => {
      qc.invalidateQueries({ queryKey: cueKeys.byTrack(trackId) });
    },
  });
}

function patchCue(
  qc: ReturnType<typeof useQueryClient>,
  cue: CuePoint,
): void {
  qc.setQueryData<CuePoint[]>(cueKeys.byTrack(cue.trackId), (old) => {
    const rest = (old ?? []).filter((c) => c.slot !== cue.slot);
    return [...rest, cue].sort((a, b) => a.slot - b.slot);
  });
}

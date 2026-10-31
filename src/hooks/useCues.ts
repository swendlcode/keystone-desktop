import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../services/keystoneService';
import type { CueEdit, CuePoint } from '../types';

export const cueKeys = {
  all: ['cues'] as const,
  byTrack: (trackId: string) => [...cueKeys.all, trackId] as const,
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

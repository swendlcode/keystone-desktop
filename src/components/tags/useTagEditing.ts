import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../../services/keystoneService';
import { trackKeys } from '../../hooks/useLibrary';
import type { CoverArt, TagEdit, TagField } from '../../types';

export const tagKeys = {
  all: ['tags'] as const,
  snapshot: (trackId: string) => ['tags', 'snapshot', trackId] as const,
  cover: (trackId: string) => ['tags', 'cover', trackId] as const,
};

/**
 * The file's own tags, read from the file rather than from the database.
 *
 * The `tracks` row holds what the indexer last saw; this is what is in the file
 * right now. For an editor those have to be the same thing, and only one of
 * them can be — a user who edited the file in another tagger should see their
 * edit here, not Keystone's memory of it.
 */
export function useTagSnapshot(trackId: string | null) {
  return useQuery({
    queryKey: trackId ? tagKeys.snapshot(trackId) : [...tagKeys.all, 'none'],
    queryFn: () => (trackId ? keystoneService.readTags(trackId) : null),
    enabled: Boolean(trackId),
  });
}

/**
 * Embedded artwork, as a base64 frame rather than a file URL.
 *
 * Cover art lives inside the tag, and the asset protocol addresses files, not
 * frames. `staleTime: Infinity` because the table renders a thumbnail per
 * visible row and these are tens of KB each.
 */
export function useCoverArt(trackId: string | null) {
  return useQuery({
    queryKey: trackId ? tagKeys.cover(trackId) : [...tagKeys.all, 'cover', 'none'],
    queryFn: () => (trackId ? keystoneService.getCoverArt(trackId) : null),
    enabled: Boolean(trackId),
    staleTime: Infinity,
  });
}

/** One field, or several, written to one file. */
export function useWriteTags() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ trackId, edit }: { trackId: string; edit: TagEdit }) =>
      keystoneService.writeTags(trackId, edit),
    onSuccess: (_report, { trackId }) => {
      qc.invalidateQueries({ queryKey: tagKeys.snapshot(trackId) });
      // The table reads the row, not the snapshot, and the row is what the
      // write just changed on disk.
      qc.invalidateQueries({ queryKey: trackKeys.all });
    },
  });
}

/** Convenience for an in-place cell edit: one field, cleared when emptied. */
export function useWriteField() {
  const write = useWriteTags();
  return (trackId: string, field: TagField, value: string) =>
    write.mutate({
      trackId,
      // `[field, null]` clears. An emptied cell means "remove this tag", which
      // is different from writing an empty string into it.
      edit: { fields: [[field, value.trim() === '' ? null : value]] },
    });
}

export function useSetCoverArt() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ trackId, art }: { trackId: string; art: CoverArt }) =>
      keystoneService.setCoverArt(trackId, art),
    onSuccess: (_r, { trackId }) => {
      qc.invalidateQueries({ queryKey: tagKeys.cover(trackId) });
      qc.invalidateQueries({ queryKey: trackKeys.all });
    },
  });
}

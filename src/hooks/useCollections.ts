import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../services/keystoneService';
import { useCollectionStore } from '../stores/collectionStore';
import { trackKeys } from './useLibrary';
import type { Collection } from '../types';

export const collectionKeys = {
  all: ['collections'] as const,
};

/**
 * The sidebar's collections, in `position` order.
 *
 * While a reorder is in flight the store's `pendingOrder` is applied over the
 * query's rows, so the list renders the dropped order immediately instead of
 * snapping back for one frame.
 */
export function useCollections() {
  const pendingOrder = useCollectionStore((s) => s.pendingOrder);
  const query = useQuery({
    queryKey: collectionKeys.all,
    queryFn: () => keystoneService.getCollections(),
  });

  if (!query.data || !pendingOrder) return query;

  const byId = new Map(query.data.map((c) => [c.id, c]));
  const reordered = pendingOrder
    .map((id) => byId.get(id))
    .filter((c): c is Collection => Boolean(c));
  // Anything the pending order did not mention keeps its place at the end,
  // so a collection created mid-drag cannot vanish from the list.
  const rest = query.data.filter((c) => !pendingOrder.includes(c.id));
  return { ...query, data: [...reordered, ...rest] };
}

export function useCreateCollection() {
  const qc = useQueryClient();
  const setCreating = useCollectionStore((s) => s.setCreating);
  return useMutation({
    mutationFn: (name: string) => keystoneService.createCollection(name),
    onSuccess: () => {
      setCreating(false);
      qc.invalidateQueries({ queryKey: collectionKeys.all });
    },
  });
}

export function useRenameCollection() {
  const qc = useQueryClient();
  const stopRename = useCollectionStore((s) => s.stopRename);
  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      keystoneService.renameCollection(id, name),
    onSuccess: () => {
      stopRename();
      qc.invalidateQueries({ queryKey: collectionKeys.all });
    },
  });
}

export function useDeleteCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => keystoneService.deleteCollection(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: collectionKeys.all });
      qc.invalidateQueries({ queryKey: trackKeys.all });
    },
  });
}

export function useReorderCollections() {
  const qc = useQueryClient();
  const setPendingOrder = useCollectionStore((s) => s.setPendingOrder);
  return useMutation({
    mutationFn: (ids: string[]) => {
      setPendingOrder(ids);
      return keystoneService.reorderCollections(ids);
    },
    // The backend rewrites `position` and answers with nothing, so the list has
    // to be refetched. `pendingOrder` is what keeps the dropped order on screen
    // until it lands, which is why this does not read as a flicker.
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: collectionKeys.all });
    },
    // Clears on failure too: the query's own order is then the truth, and
    // leaving a stale pendingOrder would pin the list to an order the
    // backend rejected.
    onSettled: () => setPendingOrder(null),
  });
}

export function useAddToCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      collectionId,
      trackIds,
    }: {
      collectionId: string;
      trackIds: string[];
    }) => keystoneService.addToCollection(collectionId, trackIds),
    onSuccess: () => {
      // The row counts changed, and so may the current page if the user is
      // looking at that collection.
      qc.invalidateQueries({ queryKey: collectionKeys.all });
      qc.invalidateQueries({ queryKey: trackKeys.all });
    },
  });
}

export function useRemoveFromCollection() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      collectionId,
      trackIds,
    }: {
      collectionId: string;
      trackIds: string[];
    }) => keystoneService.removeFromCollection(collectionId, trackIds),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: collectionKeys.all });
      qc.invalidateQueries({ queryKey: trackKeys.all });
    },
  });
}

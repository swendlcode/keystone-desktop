import { create } from 'zustand';

/**
 * Client state *around* collections: which row is being renamed, which is a
 * drop target, whether the create field is open.
 *
 * The collections themselves are server state and live in react-query under
 * `collectionKeys.all`. Mixing the two is what makes a renamed row flicker
 * back to its old name when the query refetches.
 */
interface CollectionStore {
  /** Id of the row with its name field open, or null. */
  renamingId: string | null;
  /** The `+` on the Collections header opened the new-collection field. */
  creating: boolean;
  /** Id of the row a drag is currently over, for the drop highlight. */
  dropTargetId: string | null;
  /** Ids mid-reorder, so the list can render the moved order before the
      command returns. Null when no reorder is in flight. */
  pendingOrder: string[] | null;

  startRename: (id: string) => void;
  stopRename: () => void;
  setCreating: (creating: boolean) => void;
  setDropTarget: (id: string | null) => void;
  setPendingOrder: (ids: string[] | null) => void;
}

export const useCollectionStore = create<CollectionStore>((set) => ({
  renamingId: null,
  creating: false,
  dropTargetId: null,
  pendingOrder: null,

  startRename: (renamingId) => set({ renamingId, creating: false }),
  stopRename: () => set({ renamingId: null }),
  setCreating: (creating) => set({ creating, renamingId: null }),
  setDropTarget: (dropTargetId) => set({ dropTargetId }),
  setPendingOrder: (pendingOrder) => set({ pendingOrder }),
}));

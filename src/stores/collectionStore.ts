import { create } from 'zustand';

/**
 * Client state *around* collections: which row is being renamed, whether the
 * create field is open, and the order a reorder is optimistically showing.
 *
 * Nothing about the drag itself lives here any more — that is `dragStore`,
 * which owns the one session the whole window shares.
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
  /** Ids mid-reorder, so the list can render the moved order before the
      command returns. Null when no reorder is in flight. */
  pendingOrder: string[] | null;

  startRename: (id: string) => void;
  stopRename: () => void;
  setCreating: (creating: boolean) => void;
  setPendingOrder: (ids: string[] | null) => void;
}

export const useCollectionStore = create<CollectionStore>((set) => ({
  renamingId: null,
  creating: false,
  pendingOrder: null,

  startRename: (renamingId) => set({ renamingId, creating: false }),
  stopRename: () => set({ renamingId: null }),
  setCreating: (creating) => set({ creating, renamingId: null }),
  setPendingOrder: (pendingOrder) => set({ pendingOrder }),
}));

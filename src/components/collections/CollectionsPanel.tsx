import { useState } from 'react';
import { Add, MusicSquare } from '../ui/icons';
import { ScrollArea } from '../ui/ScrollArea';
import { ConfirmDialog } from '../settings/ConfirmDialog';
import { Menu } from '../menu/Menu';
import { CollectionRow } from './CollectionRow';
import { collectionMenuItems } from './useCollectionMenu';
import { NewCollectionField } from './NewCollectionField';
import { useCollectionStore } from '../../stores/collectionStore';
import { useLibraryStore } from '../../stores/libraryStore';
import { useLibraryTotal } from '../../hooks/useLibrary';
import { formatCount } from '../../utils/formatters';
import {
  useAddToCollection,
  useCollections,
  useCreateCollection,
  useDeleteCollection,
  useRenameCollection,
  useReorderCollections,
} from '../../hooks/useCollections';
import type { Collection } from '../../types';

/**
 * The sidebar below the wheel: the collections.
 *
 * The analysis queue used to sit above them and no longer does — it is a
 * transient and lives on the status bar now, where it costs nothing while
 * nothing is running. See `layout/AnalysisStatus`.
 */
export function CollectionsPanel() {
  const { data: collections = [] } = useCollections();
  const selectedId = useLibraryStore((s) => s.filters.collectionId);
  const selectCollection = useLibraryStore((s) => s.selectCollection);
  const creating = useCollectionStore((s) => s.creating);
  const setCreating = useCollectionStore((s) => s.setCreating);
  const dragging = useCollectionStore((s) => s.draggingTracks);

  const create = useCreateCollection();
  const rename = useRenameCollection();
  const remove = useDeleteCollection();
  const reorder = useReorderCollections();
  const addTracks = useAddToCollection();

  const [pendingDelete, setPendingDelete] = useState<Collection | null>(null);
  // One menu at a time, owned here rather than per row.
  const [menu, setMenu] = useState<{ x: number; y: number; collection: Collection } | null>(
    null,
  );

  /** Move `draggedId` to where `beforeId` sits and send the whole order. */
  const onReorder = (draggedId: string, beforeId: string) => {
    const ids = collections.map((c) => c.id).filter((id) => id !== draggedId);
    const at = ids.indexOf(beforeId);
    if (at === -1) return;
    ids.splice(at, 0, draggedId);
    reorder.mutate(ids);
  };

  return (
    <>
      <PanelHeader onNew={() => setCreating(true)} />

      {/* Ringed while a drag is in the air, so the destination is visible
          before the pointer reaches it rather than only once it is on top of
          a row. The drag worked before this; nothing on screen said so. */}
      <AllTracksRow />

      <ScrollArea
        className={`min-h-0 flex-1 px-3 pb-3 transition-colors ${
          dragging ? 'rounded-md bg-keystone-lime/[0.06] ring-1 ring-inset ring-keystone-lime/40' : ''
        }`}
      >
        {creating && <NewCollectionField onCreate={(name) => create.mutate(name)} />}

        {dragging && collections.length === 0 && (
          <p className="mt-2 px-1 text-[11px] leading-relaxed text-gray-400">
            No collections yet — make one with + above, then drop onto it.
          </p>
        )}

        <div className="mt-1 flex flex-col gap-0.5">
          {collections.map((c) => (
            <CollectionRow
              key={c.id}
              collection={c}
              active={selectedId === c.id}
              onSelect={() => selectCollection(selectedId === c.id ? null : c.id)}
              onRename={(name) => rename.mutate({ id: c.id, name })}
              onAddTracks={(trackIds) => addTracks.mutate({ collectionId: c.id, trackIds })}
              onReorder={onReorder}
              onMenu={(x, y) => setMenu({ x, y, collection: c })}
            />
          ))}
        </div>
      </ScrollArea>

      {menu && (
        <Menu
          x={menu.x}
          y={menu.y}
          items={collectionMenuItems({
            collection: menu.collection,
            onDelete: () => setPendingDelete(menu.collection),
          })}
          onClose={() => setMenu(null)}
        />
      )}

      {pendingDelete && (
        <DeleteDialog
          collection={pendingDelete}
          busy={remove.isPending}
          onCancel={() => setPendingDelete(null)}
          onConfirm={() => {
            remove.mutate(pendingDelete.id);
            setPendingDelete(null);
          }}
        />
      )}
    </>
  );
}

/**
 * The section head: the word, and a filled + that reads as a button.
 *
 * The + used to be a bare glyph the same colour as the label beside it,
 * which made the one action in this panel the quietest thing in it. Filled,
 * it is findable without hovering to see whether it lights up.
 */
/**
 * The way back to the whole library.
 *
 * There was none. Choosing a collection or a key narrowed the table, and
 * getting back meant clicking the lit collection a second time, or finding
 * the chip in the toolbar, or Clear all — three different gestures for one
 * idea, none of them in the place the narrowing happened. A row at the top
 * of the list, lit whenever nothing else is, says where you are as well as
 * taking you there.
 *
 * Its count is the unfiltered total on purpose. A number that shrank as you
 * filtered would be the one number on screen that cannot tell you how much
 * there is to go back to.
 */
function AllTracksRow() {
  const collectionId = useLibraryStore((s) => s.filters.collectionId);
  const resetFilters = useLibraryStore((s) => s.resetFilters);
  const selectCollection = useLibraryStore((s) => s.selectCollection);
  const { data: total } = useLibraryTotal();

  const active = collectionId == null;

  return (
    <div className="px-3 pt-1">
      <button
        type="button"
        onClick={() => {
          // Both, because "all tracks" has to mean all of them: leaving the
          // key filter or a search in place would hand back a view that is
          // still narrowed while claiming not to be.
          selectCollection(null);
          resetFilters();
        }}
        className={`group/row flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-xs transition-colors ${
          active
            ? 'bg-gray-700 text-keystone-white'
            : 'text-gray-300 hover:bg-gray-800 hover:text-keystone-white'
        }`}
      >
        <MusicSquare size={13} color="currentColor" variant="Linear" />
        <span className="min-w-0 flex-1 truncate text-left font-medium">
          All tracks
        </span>
        <span
          className={`mono tnum shrink-0 rounded px-1.5 py-0.5 text-[11px] leading-none transition-colors ${
            active
              ? 'bg-keystone-lime text-[color:var(--color-on-accent)]'
              : 'bg-gray-700 text-gray-300 group-hover/row:bg-gray-600 group-hover/row:text-keystone-white'
          }`}
        >
          {total == null ? '—' : formatCount(total)}
        </span>
      </button>
    </div>
  );
}

function PanelHeader({ onNew }: { onNew: () => void }) {
  const dragging = useCollectionStore((s) => s.draggingTracks);
  return (
    <div className="flex items-center justify-between px-3 pb-1 pt-3">
      <span className="label">
        {dragging
          ? `Drop ${dragging} ${dragging === 1 ? 'track' : 'tracks'} here`
          : 'Collections'}
      </span>
      <button
        type="button"
        aria-label="New collection"
        title="New collection"
        onClick={onNew}
        className="flex h-5 w-5 items-center justify-center rounded bg-keystone-lime text-[color:var(--color-on-accent)] transition-colors hover:bg-[color:var(--color-accent-hover)]"
      >
        <Add size={13} color="currentColor" variant="Linear" />
      </button>
    </div>
  );
}

function DeleteDialog({
  collection,
  busy,
  onCancel,
  onConfirm,
}: {
  collection: Collection;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <ConfirmDialog
      title={`Delete ${collection.name}?`}
      destructive
      confirmLabel="Delete"
      // Worth saying out loud. Every other destructive action in Keystone
      // touches files, and someone who has learned to read these dialogs
      // carefully should be told when one does not.
      body="The collection goes; the tracks and their files stay."
      busy={busy}
      onCancel={onCancel}
      onConfirm={onConfirm}
    />
  );
}

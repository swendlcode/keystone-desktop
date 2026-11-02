import { useState, type DragEvent } from 'react';
import { Flash } from '../ui/icons';
import { useCollectionStore } from '../../stores/collectionStore';
import { formatCount } from '../../utils/formatters';
import { COLLECTION_ID_MIME, hasType, readTrackDrag, TRACK_IDS_MIME } from './dragTypes';
import type { Collection } from '../../types';

interface CollectionRowProps {
  collection: Collection;
  active: boolean;
  onSelect: () => void;
  onAddTracks: (trackIds: string[]) => void;
  onReorder: (draggedId: string, beforeId: string) => void;
  /** Right-click. The panel owns the menu so only one can be open. */
  onMenu: (x: number, y: number) => void;
}

/**
 * One sidebar row: name, count, and both drop behaviours.
 *
 * Smart collections carry a glyph and refuse track drops, because their
 * membership is a rule rather than rows — dropping onto one would appear to
 * work and then not. An ordinary collection carries none: an icon on every
 * row is decoration, and once every row has one it stops marking the row
 * that is different.
 *
 * The count is a filled pill rather than grey text. It is the only number in
 * the sidebar and the thing the eye goes to when picking a crate, so it gets
 * to be an object. The selected row takes a neutral highlight and lets the
 * pill carry the accent — tinting the whole row accent as well left two
 * things competing to say "this one".
 */
export function CollectionRow({
  collection,
  active,
  onSelect,
  onAddTracks,
  onReorder,
  onMenu,
}: CollectionRowProps) {
  const smart = collection.kind === 'smart';
  const drop = useRowDrop({ id: collection.id, smart, onAddTracks, onReorder });

  return (
    <div
      draggable
      onDragStart={(e) => e.dataTransfer.setData(COLLECTION_ID_MIME, collection.id)}
      onDragOver={drop.onDragOver}
      onDragLeave={drop.onDragLeave}
      onDrop={drop.onDrop}
      onContextMenu={(e) => {
        e.preventDefault();
        onMenu(e.clientX, e.clientY);
      }}
      className={`group/row flex items-center gap-2 rounded-md px-2.5 py-2 text-xs transition-colors ${
        drop.over === 'tracks'
          ? 'bg-keystone-lime/20 text-keystone-lime ring-1 ring-inset ring-keystone-lime/50'
          : drop.over === 'order'
            ? 'bg-gray-700'
            : active
              ? 'bg-gray-700 text-keystone-white'
              : 'text-gray-300 hover:bg-gray-800 hover:text-keystone-white'
      }`}
    >
      <RowButton
        collection={collection}
        smart={smart}
        active={active}
        onSelect={onSelect}
      />
    </div>
  );
}

function RowButton({
  collection,
  smart,
  active,
  onSelect,
}: {
  collection: Collection;
  smart: boolean;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      // No rename on double-click. That gesture is also "I meant to click and
      // the pointer moved", and a collection the user was only selecting
      // should not become editable because of it. Renaming is in the
      // right-click menu, where it is deliberate and discoverable.
      className="flex min-w-0 flex-1 items-center gap-2 text-left"
    >
      {smart && <Flash size={13} color="currentColor" variant="Linear" />}
      <span className="min-w-0 flex-1 truncate font-medium">
        {collection.name}
      </span>
      <span
        className={`mono tnum shrink-0 rounded px-1.5 py-0.5 text-[11px] leading-none transition-colors ${
          active
            ? 'bg-keystone-lime text-[color:var(--color-on-accent)]'
            : 'bg-gray-700 text-gray-300 group-hover/row:bg-gray-600 group-hover/row:text-keystone-white'
        }`}
      >
        {formatCount(collection.trackCount)}
      </span>
    </button>
  );
}

/**
 * The row's two drop behaviours, kept together because they are one decision.
 *
 * `getData` returns nothing during `dragover` — only the type list is readable
 * — so which payload is coming has to be told from `dataTransfer.types`, and
 * the drop highlight depends on getting that right before the drop lands.
 */
function useRowDrop({
  id,
  smart,
  onAddTracks,
  onReorder,
}: {
  id: string;
  smart: boolean;
  onAddTracks: (trackIds: string[]) => void;
  onReorder: (draggedId: string, beforeId: string) => void;
}) {
  const [over, setOver] = useState<null | 'tracks' | 'order'>(null);

  const onDragOver = (e: DragEvent) => {
    // A smart collection's membership is a rule, so a track drop on one would
    // appear to work and then not.
    if (hasType(e.dataTransfer, TRACK_IDS_MIME) && !smart) {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'copy';
      setOver('tracks');
    } else if (hasType(e.dataTransfer, COLLECTION_ID_MIME)) {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      setOver('order');
    }
  };

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setOver(null);
    useCollectionStore.getState().setDraggingTracks(null);
    const dragged = e.dataTransfer.getData(COLLECTION_ID_MIME);
    if (dragged && dragged !== id) {
      onReorder(dragged, id);
      return;
    }
    const ids = readTrackDrag(e.dataTransfer);
    if (ids.length && !smart) onAddTracks(ids);
  };

  return { over, onDragOver, onDragLeave: () => setOver(null), onDrop };
}

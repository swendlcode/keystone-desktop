import { useState, type DragEvent } from 'react';
import { Flash, Music, Trash } from '../ui/icons';
import { Input } from '../ui/Input';
import { Tooltip } from '../ui/Tooltip';
import { useCollectionStore } from '../../stores/collectionStore';
import { formatCount } from '../../utils/formatters';
import { COLLECTION_ID_MIME, hasType, readTrackDrag, TRACK_IDS_MIME } from './dragTypes';
import type { Collection } from '../../types';

interface CollectionRowProps {
  collection: Collection;
  active: boolean;
  onSelect: () => void;
  onRename: (name: string) => void;
  onDelete: () => void;
  onAddTracks: (trackIds: string[]) => void;
  onReorder: (draggedId: string, beforeId: string) => void;
}

/**
 * One sidebar row: name, count, and both drop behaviours.
 *
 * Smart collections carry a different glyph and refuse track drops, because
 * their membership is a rule rather than rows — dropping onto one would appear
 * to work and then not.
 */
export function CollectionRow({
  collection,
  active,
  onSelect,
  onRename,
  onDelete,
  onAddTracks,
  onReorder,
}: CollectionRowProps) {
  const renamingId = useCollectionStore((s) => s.renamingId);
  const smart = collection.kind === 'smart';
  const drop = useRowDrop({ id: collection.id, smart, onAddTracks, onReorder });

  if (renamingId === collection.id) {
    return <RenameField initial={collection.name} onSubmit={onRename} />;
  }

  return (
    <div
      draggable
      onDragStart={(e) => e.dataTransfer.setData(COLLECTION_ID_MIME, collection.id)}
      onDragOver={drop.onDragOver}
      onDragLeave={drop.onDragLeave}
      onDrop={drop.onDrop}
      className={`group/row flex items-center gap-2 rounded-md px-2 py-1.5 text-xs transition-colors ${
        drop.over === 'tracks'
          ? 'bg-keystone-lime/20 text-keystone-lime'
          : drop.over === 'order'
            ? 'bg-gray-700'
            : active
              ? 'bg-keystone-lime/15 text-keystone-lime'
              : 'text-gray-300 hover:bg-gray-800 hover:text-keystone-white'
      }`}
    >
      <RowButton collection={collection} smart={smart} onSelect={onSelect} />

      {!smart && (
        <Tooltip label="Delete">
          <button
            type="button"
            aria-label={`Delete ${collection.name}`}
            onClick={onDelete}
            className="flex h-5 w-5 items-center justify-center rounded text-gray-500 opacity-0 transition-opacity hover:text-red-400 group-hover/row:opacity-100"
          >
            <Trash size={12} color="currentColor" variant="Linear" />
          </button>
        </Tooltip>
      )}
    </div>
  );
}

function RowButton({
  collection,
  smart,
  onSelect,
}: {
  collection: Collection;
  smart: boolean;
  onSelect: () => void;
}) {
  const startRename = useCollectionStore((s) => s.startRename);
  return (
    <button
      type="button"
      onClick={onSelect}
      onDoubleClick={() => !smart && startRename(collection.id)}
      className="flex min-w-0 flex-1 items-center gap-2 text-left"
    >
      {smart ? (
        <Flash size={13} color="currentColor" variant="Linear" />
      ) : (
        <Music size={13} color="currentColor" variant="Linear" />
      )}
      <span className="min-w-0 flex-1 truncate">{collection.name}</span>
      <span className="mono tnum text-[11px] text-gray-500">
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

function RenameField({
  initial,
  onSubmit,
}: {
  initial: string;
  onSubmit: (name: string) => void;
}) {
  const [value, setValue] = useState(initial);
  const stopRename = useCollectionStore((s) => s.stopRename);

  const commit = () => {
    const name = value.trim();
    // An empty name leaves an unclickable blank row, so a cleared field is a
    // cancel rather than a rename to "".
    if (name && name !== initial) onSubmit(name);
    else stopRename();
  };

  return (
    <Input
      autoFocus
      value={value}
      className="h-8"
      onChange={(e) => setValue(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') commit();
        if (e.key === 'Escape') stopRename();
      }}
    />
  );
}

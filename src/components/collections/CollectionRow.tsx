import { useEffect, useRef } from 'react';
import { Flash } from '../ui/icons';
import { formatCount } from '../../utils/formatters';
import {
  armPointerDrag,
  DROP_ZONE_ATTR,
  registerDropZone,
  useDragStore,
  type DragPayload,
} from '../../stores/dragStore';
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
 *
 * The outer element is the drop zone and carries the gap to the row below it
 * as padding rather than letting the list space the rows. Under `gap-0.5`
 * there were two pixels between every pair of rows that belonged to neither,
 * and a drop aimed at the boundary landed on nothing at all.
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
  const over = useDragStore((s) => s.overId === collection.id);
  const incoming = useDragStore((s) => s.payload?.kind ?? null);

  // The zone is registered for the row's lifetime, but the handlers it calls
  // are fresh closures on every render. A ref keeps the registration stable
  // while the behaviour stays current.
  const handlers = useRef({ onAddTracks, onReorder });
  handlers.current = { onAddTracks, onReorder };

  useEffect(
    () =>
      registerDropZone(collection.id, {
        accepts: smart ? ['collection'] : ['tracks', 'collection'],
        onDrop: (payload) => {
          if (payload.kind === 'tracks') {
            handlers.current.onAddTracks(payload.trackIds);
            return;
          }
          // Dropping a collection on itself is a cancelled reorder.
          if (payload.collectionId !== collection.id) {
            handlers.current.onReorder(payload.collectionId, collection.id);
          }
        },
      }),
    [collection.id, smart],
  );

  return (
    <div {...{ [DROP_ZONE_ATTR]: collection.id }} className="pb-0.5">
      <div
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          armPointerDrag(
            e.clientX,
            e.clientY,
            (): DragPayload => ({ kind: 'collection', collectionId: collection.id }),
          );
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          onMenu(e.clientX, e.clientY);
        }}
        className={`group/row flex items-center gap-2 rounded-md px-2.5 py-2 text-xs transition-colors ${
          over && incoming === 'tracks'
            ? 'bg-keystone-lime/20 text-keystone-lime ring-1 ring-inset ring-keystone-lime/50'
            : over && incoming === 'collection'
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

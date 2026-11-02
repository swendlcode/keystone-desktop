import { save, open as openDialog } from '@tauri-apps/plugin-dialog';
import { Add, DocumentDownload, Edit2, Export, Trash } from '../ui/icons';
import { keystoneService } from '../../services/keystoneService';
import { useCollectionStore } from '../../stores/collectionStore';
import type { Collection, Track } from '../../types';
import type { MenuItem } from '../ui/menuItem';

/** One request's worth of rows. Collections are not libraries. */
const PAGE = 2000;

/**
 * The right-click menu on a collection row.
 *
 * It replaces a trash button that appeared on hover. Two problems with that
 * button: it was the only action a collection had, so renaming was a
 * double-click nobody discovers, and a delete control that materialises
 * under a moving pointer is the one control that should never be easy to
 * hit by accident. A menu makes every action equally reachable and puts the
 * destructive one behind a deliberate gesture.
 */
export function collectionMenuItems({
  collection,
  onDelete,
}: {
  collection: Collection;
  onDelete: () => void;
}): MenuItem[] {
  const store = useCollectionStore.getState();
  const smart = collection.kind === 'smart';

  return [
    {
      id: 'add',
      label: 'Add Collection',
      icon: <Add size={12} color="currentColor" variant="Linear" />,
      onSelect: () => store.setCreating(true),
    },
    {
      id: 'rename',
      label: 'Rename Collection',
      icon: <Edit2 size={12} color="currentColor" variant="Linear" />,
      // A smart collection's name is part of its rule, not a label.
      disabled: smart,
      onSelect: () => store.startRename(collection.id),
    },
    {
      id: 'remove',
      label: 'Remove Collection',
      icon: <Trash size={12} color="currentColor" variant="Linear" />,
      danger: true,
      disabled: smart,
      onSelect: onDelete,
    },
    {
      id: 'export-files',
      label: 'Export Files...',
      icon: <Export size={12} color="currentColor" variant="Linear" />,
      separatorBefore: true,
      onSelect: () => void exportFiles(collection),
    },
    {
      id: 'export-sheet',
      label: 'Export Collection to CSV...',
      icon: <DocumentDownload size={12} color="currentColor" variant="Linear" />,
      onSelect: () => void exportSheet(collection),
    },
  ];
}

/** Every track in the collection, copied into a folder the user picks. */
async function exportFiles(collection: Collection): Promise<void> {
  const dest = await openDialog({ directory: true, multiple: false });
  if (typeof dest !== 'string') return;

  const tracks = await collectionTracks(collection.id);
  for (const track of tracks) {
    try {
      await keystoneService.exportFile(track.id, dest);
    } catch (e) {
      // One unreadable file must not abandon the other four hundred. The
      // copy is non-destructive, so carrying on is the safe direction.
      console.error('[collection] could not export', track.path, e);
    }
  }
}

/**
 * The collection as a spreadsheet.
 *
 * Written in Rust. The frontend cannot write to a path of the user's
 * choosing — the fs plugin's default scope does not reach outside the app's
 * own directories, and widening it to arbitrary paths to save one file
 * would be a far larger permission than this feature needs.
 */
async function exportSheet(collection: Collection): Promise<void> {
  const dest = await save({
    defaultPath: `${safeName(collection.name)}.csv`,
    filters: [{ name: 'CSV', extensions: ['csv'] }],
  });
  if (typeof dest !== 'string') return;
  await keystoneService.exportCollectionCsv(collection.id, dest);
}

async function collectionTracks(collectionId: string): Promise<Track[]> {
  const out: Track[] = [];
  for (let offset = 0; ; offset += PAGE) {
    const page = await keystoneService.getCollectionPage({
      collectionId,
      sort: 'artist',
      direction: 'asc',
      limit: PAGE,
      offset,
    });
    out.push(...page.tracks);
    if (out.length >= page.total || page.tracks.length === 0) break;
  }
  return out;
}

function safeName(name: string): string {
  return name.replace(/[\\/:*?"<>|]/g, '-').trim() || 'collection';
}

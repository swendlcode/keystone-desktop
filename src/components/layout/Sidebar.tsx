import { useState } from 'react';
import { FolderAdd } from '../ui/icons';
import { Button } from '../ui/Button';
import { CollectionsPanel } from '../collections/CollectionsPanel';
import { AddTracksDialog } from '../import/AddTracksDialog';
import { MusicImportDialog } from '../import/MusicImportDialog';
import { CamelotWheelSlot } from '../library/CamelotWheelSlot';
import { Logo } from '../ui/Logo';
import { useUiStore } from '../../stores/uiStore';
import { useAddFolder } from '../../hooks/useLibrary';
import { open } from '@tauri-apps/plugin-dialog';

/**
 * Left column: the wheel, ADD TRACKS, the analysis queue, the collections.
 *
 * Width is user-set and persisted in `uiStore`; closing it collapses the
 * column entirely rather than narrowing it, because the wheel has a minimum
 * size below which it stops being clickable.
 */
export function Sidebar() {
  const open_ = useUiStore((s) => s.sidebarOpen);
  const width = useUiStore((s) => s.sidebarWidth);
  const addFolder = useAddFolder();
  // 'choose' is the two-way fork; 'playlist' is the Music import. Held here
  // rather than in `uiStore` because nothing outside this column opens them.
  const [dialog, setDialog] = useState<null | 'choose' | 'playlist'>(null);

  if (!open_) return null;

  const pickFolder = async () => {
    setDialog(null);
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string') addFolder.mutate(picked);
  };

  return (
    <aside
      style={{ width }}
      className="flex shrink-0 flex-col border-r border-gray-700 bg-gray-900"
    >
      {/* The mark sits here rather than in the title bar, where macOS draws
          the traffic lights over anything at the left edge — that collision
          is why it moved to the status bar earlier. The sidebar's top is the
          one place in this window with nothing drawn over it and a natural
          left edge to sit against. */}
      <div className="flex items-center justify-center px-3 pb-2 pt-4">
        <Logo size={26} />
      </div>

      <CamelotWheelSlot />

      <div className="px-3 pb-3">
        <Button
          variant="primary"
          size="sm"
          className="w-full"
          onClick={() => setDialog('choose')}
          disabled={addFolder.isPending}
          icon={<FolderAdd size={14} color="currentColor" variant="Linear" />}
        >
          ADD TRACKS
        </Button>
      </div>

      <CollectionsPanel />

      {dialog === 'choose' && (
        <AddTracksDialog
          onFolder={() => void pickFolder()}
          onPlaylist={() => setDialog('playlist')}
          onCancel={() => setDialog(null)}
        />
      )}
      {dialog === 'playlist' && (
        <MusicImportDialog onClose={() => setDialog(null)} />
      )}
    </aside>
  );
}

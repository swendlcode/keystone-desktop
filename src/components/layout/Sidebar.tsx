import { FolderAdd } from '../ui/icons';
import { Button } from '../ui/Button';
import { CollectionsPanel } from '../collections/CollectionsPanel';
import { CamelotWheelSlot } from '../library/CamelotWheelSlot';
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

  if (!open_) return null;

  const pickFolder = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string') addFolder.mutate(picked);
  };

  return (
    <aside
      style={{ width }}
      className="flex shrink-0 flex-col border-r border-gray-700 bg-gray-900"
    >
      <CamelotWheelSlot />

      <div className="px-3 pb-3">
        <Button
          variant="primary"
          size="sm"
          className="w-full"
          onClick={pickFolder}
          disabled={addFolder.isPending}
          icon={<FolderAdd size={14} color="currentColor" variant="Linear" />}
        >
          ADD TRACKS
        </Button>
      </div>

      {/* The queue row and the collections together: the panel owns their order
          because the queue must stay visible while the list scrolls under it. */}
      <CollectionsPanel />
    </aside>
  );
}

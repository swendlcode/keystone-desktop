import { lazy, Suspense } from 'react';
import { TitleBar } from './components/layout/TitleBar';
import { Sidebar } from './components/layout/Sidebar';
import { SidebarResizer } from './components/layout/SidebarResizer';
import { MainPanel } from './components/layout/MainPanel';
import { ViewTabs } from './components/layout/ViewTabs';
import { StatusBar } from './components/layout/StatusBar';
import { DropOverlay } from './components/layout/DropOverlay';
import { DragGhost } from './components/collections/DragGhost';
import { PlayerBar } from './components/player/PlayerBar';
import { TrackContextMenu } from './components/menu/TrackContextMenu';
import { RemoveSelectedShortcut } from './components/library/RemoveSelectedShortcut';
import { SongInfoDialog } from './components/library/SongInfoDialog';
import { useUiStore } from './stores/uiStore';
import { useLibrarySync } from './hooks/useLibrary';
import { useAnalysisSync } from './hooks/useAnalysis';
import { useKeyboard } from './hooks/useKeyboard';
import { useDragSession } from './hooks/useDragSession';

const LibraryPage = lazy(() =>
  import('./pages/LibraryPage').then((m) => ({ default: m.LibraryPage })),
);
const TagsPage = lazy(() =>
  import('./pages/TagsPage').then((m) => ({ default: m.TagsPage })),
);
const SettingsPage = lazy(() =>
  import('./pages/SettingsPage').then((m) => ({ default: m.SettingsPage })),
);

export default function App() {
  const view = useUiStore((s) => s.view);

  // Event listeners and the one audio element. Mounted here, once: a listener
  // inside a lazily loaded page would miss everything emitted before the user
  // happened to open that page.
  useLibrarySync();
  useAnalysisSync();
  useKeyboard();
  useDragSession();

  return (
    <div className="h-app flex w-full flex-col overflow-hidden bg-keystone-black">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <SidebarResizer />
        <MainPanel>
          {/* The column's own header. Library / Tags switch what this column
              shows, so they sit at the top of it rather than in the window
              bar that spans the sidebar as well. */}
          <ViewTabs />
          <PlayerBar />
          <Suspense fallback={<div className="flex-1" />}>
            {view === 'library' && <LibraryPage />}
            {view === 'tags' && <TagsPage />}
            {view === 'settings' && <SettingsPage />}
          </Suspense>
        </MainPanel>
      </div>
      <StatusBar />
      <DropOverlay />

      {/* Follows the pointer while rows are being dragged to a collection. */}
      <DragGhost />
      {/* One menu for every table: its anchor is `uiStore.contextMenu`, which
          both the library and the tag table write to. Mounted here rather than
          per table so a right-click cannot leave two open. */}
      <TrackContextMenu />
      {/* Backspace / Delete on the selection. Mounted here so one listener
          and one confirmation serve both tables. */}
      <RemoveSelectedShortcut />
      {/* Song Info. Mounted here because the context menu that opens it is
          here, and because it outlives the menu — selecting the item closes
          the menu, so a panel the menu owned would unmount in the same
          tick. */}
      <SongInfoDialog />
    </div>
  );
}

import { lazy, Suspense } from 'react';
import { TitleBar } from './components/layout/TitleBar';
import { Sidebar } from './components/layout/Sidebar';
import { MainPanel } from './components/layout/MainPanel';
import { StatusBar } from './components/layout/StatusBar';
import { DropOverlay } from './components/layout/DropOverlay';
import { PlayerBar } from './components/player/PlayerBar';
import { TrackContextMenu } from './components/menu/TrackContextMenu';
import { useUiStore } from './stores/uiStore';
import { useLibrarySync } from './hooks/useLibrary';
import { useAnalysisSync } from './hooks/useAnalysis';
import { useKeyboard } from './hooks/useKeyboard';

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

  return (
    <div className="h-app flex w-full flex-col overflow-hidden bg-keystone-black">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <MainPanel>
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
      {/* One menu for every table: its anchor is `uiStore.contextMenu`, which
          both the library and the tag table write to. Mounted here rather than
          per table so a right-click cannot leave two open. */}
      <TrackContextMenu />
    </div>
  );
}

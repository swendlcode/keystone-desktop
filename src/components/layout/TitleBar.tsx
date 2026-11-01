import { getCurrentWindow } from '@tauri-apps/api/window';
import type { MouseEvent } from 'react';
import { HamburgerMenu, Moon, SearchNormal, Setting2, Sun1, Tag } from '../ui/icons';
import { Input } from '../ui/Input';
import { useUiStore, type ActiveView } from '../../stores/uiStore';
import { useLibraryStore } from '../../stores/libraryStore';
import { useSettingsStore } from '../../stores/settingsStore';

const TABS: Array<{ id: ActiveView; label: string }> = [
  { id: 'library', label: 'Library' },
  { id: 'tags', label: 'Tags' },
];

/**
 * The window chrome: mark, view tabs, search, theme and settings.
 *
 * The drag layer is a sibling under the controls rather than the bar itself.
 * Putting `data-tauri-drag-region` on a container swallows clicks on anything
 * inside it, so the controls sit above an absolutely positioned drag surface.
 */
export function TitleBar() {
  const view = useUiStore((s) => s.view);
  const setView = useUiStore((s) => s.setView);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const query = useLibraryStore((s) => s.filters.query);
  const setQuery = useLibraryStore((s) => s.setQuery);
  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);

  const startDrag = (e: MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    getCurrentWindow().startDragging().catch(() => {
      /* not in a Tauri window — nothing to drag */
    });
  };

  // Theme is client state, persisted in localStorage rather than through the
  // settings command — see the note in types/settings.ts. Nothing to await,
  // so the flip is immediate.
  const toggleTheme = () => setTheme(theme === 'dark' ? 'light' : 'dark');

  return (
    // pl-[76px] clears the macOS traffic lights, which the Overlay title bar
    // draws at x:16 over whatever is beneath them. The mark used to sit here
    // and was rendered underneath the close button; it lives in the StatusBar
    // now, where nothing is drawn over it.
    <div className="relative flex h-12 shrink-0 items-center gap-3 border-b border-gray-700 bg-gray-900 pl-[76px] pr-3">
      <div
        data-tauri-drag-region
        onMouseDown={startDrag}
        className="absolute inset-0 z-0 cursor-default"
      />

      <div className="z-10 flex items-center gap-2">
        <button
          type="button"
          onClick={toggleSidebar}
          aria-label="Toggle sidebar"
          className="flex h-8 w-8 items-center justify-center rounded-md text-gray-400 transition-colors hover:bg-gray-800 hover:text-keystone-white"
        >
          <HamburgerMenu size={18} color="currentColor" variant="Linear" />
        </button>
      </div>

      <nav className="z-10 flex items-center gap-1">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            onClick={() => setView(tab.id)}
            className={`flex h-8 items-center gap-1.5 rounded-md px-3 text-xs font-semibold transition-colors ${
              view === tab.id
                ? 'bg-keystone-lime/15 text-keystone-lime'
                : 'text-gray-400 hover:bg-gray-800 hover:text-keystone-white'
            }`}
          >
            {tab.id === 'tags' && (
              <Tag size={13} color="currentColor" variant="Linear" />
            )}
            {tab.label}
          </button>
        ))}
      </nav>

      <div className="z-10 ml-auto flex w-full max-w-sm items-center">
        <Input
          id="global-search"
          value={query}
          placeholder="Search"
          onChange={(e) => setQuery(e.target.value)}
          leading={
            <SearchNormal size={14} color="currentColor" variant="Linear" className="text-gray-400" />
          }
          className="w-full"
        />
      </div>

      <div className="z-10 flex items-center gap-1">
        <button
          type="button"
          onClick={toggleTheme}
          aria-label="Toggle theme"
          className="flex h-8 w-8 items-center justify-center rounded-md text-gray-400 transition-colors hover:bg-gray-800 hover:text-keystone-white"
        >
          {theme === 'dark' ? (
            <Sun1 size={16} color="currentColor" variant="Linear" />
          ) : (
            <Moon size={16} color="currentColor" variant="Linear" />
          )}
        </button>
        <button
          type="button"
          onClick={() => setView('settings')}
          aria-label="Settings"
          className={`flex h-8 w-8 items-center justify-center rounded-md transition-colors ${
            view === 'settings'
              ? 'bg-keystone-lime/15 text-keystone-lime'
              : 'text-gray-400 hover:bg-gray-800 hover:text-keystone-white'
          }`}
        >
          <Setting2 size={16} color="currentColor" variant="Linear" />
        </button>
      </div>
    </div>
  );
}

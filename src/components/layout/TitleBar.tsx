import { getCurrentWindow } from '@tauri-apps/api/window';
import type { MouseEvent } from 'react';
import { Moon, Setting2, Sun1, Tag } from '../ui/icons';
import { useUiStore, type ActiveView } from '../../stores/uiStore';
import { useSettingsStore } from '../../stores/settingsStore';

const TABS: Array<{ id: ActiveView; label: string }> = [
  { id: 'library', label: 'Library' },
  { id: 'tags', label: 'Tags' },
];

/**
 * The window chrome: view tabs, theme and settings.
 *
 * Deliberately thin. It used to carry a search field as well, which put two
 * search boxes on screen at once — this one and the view's own, in the library
 * toolbar and in the tags toolbar — writing the same `filters.query`. Two
 * fields for one value is a bug whichever one you type in: the other shows
 * something stale, and neither reads as the one that matters. The view's own
 * field wins because it sits with the result count and the filter chips it
 * narrows, and because it is the one that debounces.
 *
 * The sidebar toggle is gone from here too; the sidebar's own edge handle
 * drags it narrower, shut and open again, which is where a hand already is.
 *
 * The drag layer is a sibling under the controls rather than the bar itself.
 * Putting `data-tauri-drag-region` on a container swallows clicks on anything
 * inside it, so the controls sit above an absolutely positioned drag surface.
 */
export function TitleBar() {
  const view = useUiStore((s) => s.view);
  const setView = useUiStore((s) => s.setView);
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

      <div className="z-10 ml-auto flex items-center gap-1">
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

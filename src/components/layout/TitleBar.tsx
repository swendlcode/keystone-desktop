import { getCurrentWindow } from '@tauri-apps/api/window';
import type { MouseEvent } from 'react';
import { Moon, Setting2, Sun1 } from '../ui/icons';
import { useUiStore } from '../../stores/uiStore';
import { useSettingsStore } from '../../stores/settingsStore';

/**
 * The window chrome: theme and settings, and the drag surface.
 *
 * Deliberately thin, and it keeps getting thinner. It used to carry a search
 * field, which put two search boxes on screen writing the same
 * `filters.query` — two fields for one value is a bug whichever one you type
 * in. It used to carry a sidebar toggle, which the sidebar's own edge handle
 * does better. And it used to carry the Library / Tags tabs, which govern the
 * second column and now live at the top of it.
 *
 * What is left is window-wide: the theme, and the settings view that replaces
 * everything.
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
    // The macOS traffic lights are drawn over the Overlay title bar from x:16
    // and run to about x:70. 76px cleared them arithmetically and still read
    // as crowded — the first tab sat four pixels off the zoom button with no
    // gutter between two unrelated groups of controls. 100px gives the tabs a
    // margin of their own rather than the minimum that avoids a collision.
    <div className="relative flex h-10 shrink-0 items-center gap-3 border-b border-gray-700 bg-gray-900 pl-[100px] pr-3">
      <div
        data-tauri-drag-region
        onMouseDown={startDrag}
        className="absolute inset-0 z-0 cursor-default"
      />

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

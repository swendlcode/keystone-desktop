import { create } from 'zustand';
import type { Settings, Theme } from '../types';

const THEME_KEY = 'keystone:theme';

/**
 * Theme, read and applied before React mounts.
 *
 * The backend does not store the theme — see the note in types/settings.ts —
 * and it could not help here anyway: a first paint in dark followed by a swap
 * to light is the flash everyone notices, and a SQLite round trip cannot beat
 * the first paint. So it is mirrored into localStorage and read synchronously
 * from main.tsx.
 */
export function readPersistedTheme(): Theme {
  try {
    return localStorage.getItem(THEME_KEY) === 'light' ? 'light' : 'dark';
  } catch {
    // Private mode, or site data blocked. Dark is the documented default.
    return 'dark';
  }
}

export function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute('data-theme', theme);
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch {
    /* the attribute is already set; persistence is the only loss */
  }
}

/** The left rail of SPEC.md §3, in order. */
export type SettingsSection =
  | 'personalize'
  | 'library'
  | 'feedback'
  | 'updateTags'
  | 'exportCuePoints'
  | 'renameFiles'
  | 'keyNotation'
  | 'tempo'
  | 'audioPlayer'
  | 'displayOptions'
  | 'about';

export const SETTINGS_SECTIONS: Array<{ id: SettingsSection; label: string }> = [
  { id: 'personalize', label: 'Personalize' },
  // Second, above the panes that write to files: it is about what is *in* the
  // library, which is the question a user has before any of those.
  { id: 'library', label: 'Library' },
  { id: 'feedback', label: 'Send Feedback' },
  { id: 'updateTags', label: 'Update Tags' },
  { id: 'exportCuePoints', label: 'Export Cue Points' },
  { id: 'renameFiles', label: 'Rename Files' },
  { id: 'keyNotation', label: 'Key Notation' },
  { id: 'tempo', label: 'Tempo' },
  { id: 'audioPlayer', label: 'Audio Player' },
  { id: 'displayOptions', label: 'Display Options' },
  { id: 'about', label: 'About' },
];

/**
 * Which section is open, the theme, and the dirty-field buffer.
 *
 * The settings *values* are not here. A second copy of a forty-field object
 * that also lives in react-query drifts the moment one write fails.
 */
interface SettingsStore {
  section: SettingsSection;
  theme: Theme;
  draft: Partial<Settings>;

  setSection: (section: SettingsSection) => void;
  setTheme: (theme: Theme) => void;
  patchDraft: (patch: Partial<Settings>) => void;
  clearDraft: () => void;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  section: 'personalize',
  theme: readPersistedTheme(),
  draft: {},

  // Switching section drops the draft: a half-typed BPM range left behind in
  // another section would be saved by the next section's save button.
  setSection: (section) => set({ section, draft: {} }),

  setTheme: (theme) => {
    applyTheme(theme);
    set({ theme });
  },

  patchDraft: (patch) => set((s) => ({ draft: { ...s.draft, ...patch } })),
  clearDraft: () => set({ draft: {} }),
}));

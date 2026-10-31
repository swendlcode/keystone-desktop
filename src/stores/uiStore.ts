import { create } from 'zustand';

/** The three views. SPEC.md's main window, tag editing and settings. */
export type ActiveView = 'library' | 'tags' | 'settings';

const SIDEBAR_WIDTH_KEY = 'keystone:sidebarWidth';
const SIDEBAR_OPEN_KEY = 'keystone:sidebarOpen';
const MIN_WIDTH = 200;
const MAX_WIDTH = 460;
const DEFAULT_WIDTH = 260;

function loadWidth(): number {
  try {
    const v = localStorage.getItem(SIDEBAR_WIDTH_KEY);
    if (v) {
      const n = parseInt(v, 10);
      if (n >= MIN_WIDTH && n <= MAX_WIDTH) return n;
    }
  } catch {
    /* storage blocked — fall through to the default */
  }
  return DEFAULT_WIDTH;
}

function loadOpen(): boolean {
  try {
    const v = localStorage.getItem(SIDEBAR_OPEN_KEY);
    if (v !== null) return v === 'true';
  } catch {
    /* as above */
  }
  return true;
}

interface UiStore {
  view: ActiveView;
  sidebarOpen: boolean;
  sidebarWidth: number;
  /** Track id whose Song Info panel is open, or null. */
  infoTrackId: string | null;
  /** The virtual piano panel, which shows the detected key's scale. */
  pianoOpen: boolean;
  /** Context menu anchor, or null when closed. One menu at a time. */
  contextMenu: { x: number; y: number; trackId: string } | null;

  setView: (view: ActiveView) => void;
  toggleSidebar: () => void;
  setSidebarWidth: (width: number) => void;
  openInfo: (trackId: string) => void;
  closeInfo: () => void;
  togglePiano: () => void;
  openContextMenu: (menu: NonNullable<UiStore['contextMenu']>) => void;
  closeContextMenu: () => void;
}

export const useUiStore = create<UiStore>((set) => ({
  view: 'library',
  sidebarOpen: loadOpen(),
  sidebarWidth: loadWidth(),
  infoTrackId: null,
  pianoOpen: false,
  contextMenu: null,

  setView: (view) => set({ view, contextMenu: null }),

  toggleSidebar: () =>
    set((s) => {
      const next = !s.sidebarOpen;
      try {
        localStorage.setItem(SIDEBAR_OPEN_KEY, String(next));
      } catch {
        /* unpersisted, still toggled */
      }
      return { sidebarOpen: next };
    }),

  setSidebarWidth: (width) => {
    const clamped = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, width));
    try {
      localStorage.setItem(SIDEBAR_WIDTH_KEY, String(clamped));
    } catch {
      /* unpersisted, still applied */
    }
    set({ sidebarWidth: clamped });
  },

  openInfo: (infoTrackId) => set({ infoTrackId }),
  closeInfo: () => set({ infoTrackId: null }),
  togglePiano: () => set((s) => ({ pianoOpen: !s.pianoOpen })),
  openContextMenu: (contextMenu) => set({ contextMenu }),
  closeContextMenu: () => set({ contextMenu: null }),
}));

export { MIN_WIDTH, MAX_WIDTH };

import { create } from 'zustand';
import { COLUMN_ORDER, type TrackColumn } from '../types';

/** The three views. SPEC.md's main window, tag editing and settings. */
export type ActiveView = 'library' | 'tags' | 'settings';

const COLUMN_ORDER_KEY = 'keystone:columnOrder';
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

/**
 * The saved column order, reconciled against the columns that exist.
 *
 * Unknown names are dropped and missing ones appended in their default
 * position, so a column added in a later release appears for a user who
 * already has an order saved — rather than being invisible until they reset
 * their layout, which is the failure mode of trusting a stored list.
 */
function loadColumnOrder(): TrackColumn[] {
  let saved: unknown;
  try {
    const raw = localStorage.getItem(COLUMN_ORDER_KEY);
    saved = raw ? JSON.parse(raw) : null;
  } catch {
    saved = null;
  }
  if (!Array.isArray(saved)) return [...COLUMN_ORDER];

  const known = new Set<string>(COLUMN_ORDER);
  const seen = new Set<string>();
  const order: TrackColumn[] = [];
  for (const c of saved) {
    if (typeof c === 'string' && known.has(c) && !seen.has(c)) {
      seen.add(c);
      order.push(c as TrackColumn);
    }
  }
  for (const c of COLUMN_ORDER) if (!seen.has(c)) order.push(c);
  return order;
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
  /** Column order, left to right. Visibility is a separate Setting. */
  columnOrder: TrackColumn[];
  /** Track id whose Song Info panel is open, or null. */
  infoTrackId: string | null;
  /** The virtual piano panel, which shows the detected key's scale. */
  pianoOpen: boolean;
  /** Context menu anchor, or null when closed. One menu at a time. */
  contextMenu: { x: number; y: number; trackId: string } | null;

  setView: (view: ActiveView) => void;
  toggleSidebar: () => void;
  setSidebarWidth: (width: number) => void;
  setColumnOrder: (order: TrackColumn[]) => void;
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
  columnOrder: loadColumnOrder(),
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

  setColumnOrder: (columnOrder) => {
    try {
      localStorage.setItem(COLUMN_ORDER_KEY, JSON.stringify(columnOrder));
    } catch {
      /* unpersisted, still applied */
    }
    set({ columnOrder });
  },

  openInfo: (infoTrackId) => set({ infoTrackId }),
  closeInfo: () => set({ infoTrackId: null }),
  togglePiano: () => set((s) => ({ pianoOpen: !s.pianoOpen })),
  openContextMenu: (contextMenu) => set({ contextMenu }),
  closeContextMenu: () => set({ contextMenu: null }),
}));

export { MIN_WIDTH, MAX_WIDTH };

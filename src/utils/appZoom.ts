/**
 * Interface scale.
 *
 * The whole window scales, not just type. Every dimension in this layout is a
 * px literal — the virtual piano's key widths, the Camelot wheel's radii, the
 * waveform's height, a table row — so scaling the font alone gives bigger text
 * inside the same furniture, which is a worse layout rather than a larger one.
 * This drives the webview's own zoom instead, the same mechanism a browser's
 * Cmd +/- uses, and every px in the stylesheet moves with it.
 *
 * The factor lives in localStorage rather than in the settings row the backend
 * owns, because it describes this display: a value picked on a 32" monitor is
 * the wrong one the moment the same settings are read on a laptop.
 *
 * If anything here ever converts between screen and viewport coordinates, it
 * has to divide by `currentZoom()` — `getBoundingClientRect()` reports zoomed
 * CSS pixels while the OS reports real ones. Nothing does today: the collection
 * drag is a pointer session inside the document, and the title bar hands its
 * drag straight to the window without reading a position.
 */
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

const ZOOM_KEY = 'keystone:zoom';

export const MIN_ZOOM = 0.7;
export const MAX_ZOOM = 2;
export const DEFAULT_ZOOM = 1;

/** The rungs Cmd +/- steps through — the ladder browsers use, trimmed. */
export const ZOOM_STEPS = [0.7, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];

/** Fired on every change so an open pane can follow the keyboard. */
export const ZOOM_EVENT = 'keystone:zoom-changed';

function clamp(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_ZOOM;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value));
}

let current = DEFAULT_ZOOM;

export function readZoom(): number {
  try {
    const raw = localStorage.getItem(ZOOM_KEY);
    return raw ? clamp(Number(raw)) : DEFAULT_ZOOM;
  } catch {
    return DEFAULT_ZOOM;
  }
}

/** The factor in force right now. Synchronous, for coordinate math. */
export function currentZoom(): number {
  return current;
}

export function applyZoom(factor: number): number {
  const next = clamp(factor);
  current = next;
  try {
    if (next === DEFAULT_ZOOM) localStorage.removeItem(ZOOM_KEY);
    else localStorage.setItem(ZOOM_KEY, String(next));
  } catch {
    /* private mode — the zoom still applies, it just will not be remembered */
  }
  // The CSS property is supported by the same engine and scales the same way.
  // It is the fallback, not the primary, because it reflows a little
  // differently around fixed positioning.
  const cssZoom = () => {
    document.documentElement.style.zoom = String(next);
  };
  try {
    getCurrentWebviewWindow().setZoom(next).catch(cssZoom);
  } catch {
    // `getCurrentWebviewWindow()` reads `window.__TAURI_INTERNALS__.metadata`,
    // so outside a Tauri webview it **throws synchronously** — there is no
    // promise for the `.catch` above to catch. Catching only the rejection is
    // what took Stack's browser mode down: the throw escaped startup and the
    // page never rendered a pixel.
    cssZoom();
  }
  window.dispatchEvent(new CustomEvent(ZOOM_EVENT, { detail: { zoom: next } }));
  return next;
}

/** Restore the saved factor. Called once, at startup. */
export function initZoom(): number {
  return applyZoom(readZoom());
}

function step(direction: 1 | -1): number {
  const ladder = direction === 1 ? ZOOM_STEPS : [...ZOOM_STEPS].reverse();
  const next = ladder.find((z) => (direction === 1 ? z > current + 1e-6 : z < current - 1e-6));
  return applyZoom(next ?? current);
}

export const zoomIn = () => step(1);
export const zoomOut = () => step(-1);
export const zoomReset = () => applyZoom(DEFAULT_ZOOM);

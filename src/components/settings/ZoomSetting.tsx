import { useEffect, useState } from 'react';
import { SettingGroup, SettingNote, SettingRow } from './SettingSection';
import {
  currentZoom,
  DEFAULT_ZOOM,
  readZoom,
  ZOOM_EVENT,
  zoomIn,
  zoomOut,
  zoomReset,
} from '../../utils/appZoom';

/**
 * Interface scale, with the same three controls the keyboard has.
 *
 * It listens for the zoom event rather than owning the value, so the readout
 * stays right when the shortcut is used while Settings is open.
 */
export function ZoomSetting() {
  const [zoom, setZoom] = useState(() => currentZoom() || readZoom());

  useEffect(() => {
    const onChange = (e: Event) => setZoom((e as CustomEvent<{ zoom: number }>).detail.zoom);
    window.addEventListener(ZOOM_EVENT, onChange);
    return () => window.removeEventListener(ZOOM_EVENT, onChange);
  }, []);

  const btn =
    'flex h-7 w-7 items-center justify-center rounded-md border border-gray-700 text-gray-300 ' +
    'transition-colors hover:border-keystone-lime/60 hover:text-keystone-white ' +
    'disabled:opacity-40 disabled:hover:border-gray-700 disabled:hover:text-gray-300';

  return (
    <SettingGroup label="Interface scale">
      <SettingRow label="Size of everything in the window" hint="Not just the text.">
        <span className="flex items-center gap-2">
          <button className={btn} onClick={() => setZoom(zoomOut())} aria-label="Smaller">
            <span className="text-base leading-none">−</span>
          </button>
          <span className="mono w-14 text-center text-xs text-gray-300">
            {Math.round(zoom * 100)}%
          </span>
          <button className={btn} onClick={() => setZoom(zoomIn())} aria-label="Bigger">
            <span className="text-base leading-none">+</span>
          </button>
          {/* The percentage is deliberately not the reset control: nothing
              about a number says it can be clicked. */}
          <button
            className="ml-1 flex h-7 items-center rounded-md border border-gray-700 px-2
              text-[10px] uppercase tracking-widest text-gray-400 transition-colors
              hover:border-keystone-lime/60 hover:text-keystone-white disabled:opacity-40
              disabled:hover:border-gray-700 disabled:hover:text-gray-400"
            onClick={() => setZoom(zoomReset())}
            disabled={zoom === DEFAULT_ZOOM}
            title="Back to 100%"
          >
            Reset
          </button>
        </span>
      </SettingRow>
      <SettingNote>
        Cmd / Ctrl with + and − do the same from anywhere in the app, and Cmd / Ctrl 0 goes back to
        100%. Remembered for this display only.
      </SettingNote>
    </SettingGroup>
  );
}

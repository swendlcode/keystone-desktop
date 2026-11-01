import { openUrl } from '@tauri-apps/plugin-opener';
import { getVersion } from '@tauri-apps/api/app';
import { useEffect, useState } from 'react';
import { Logo } from '../ui/Logo';
import { Coffee } from '../ui/icons';
import { AnalysisStatus } from './AnalysisStatus';

const COFFEE = 'https://www.buymeacoffee.com/swendl';

/**
 * The byline strip along the bottom: mark, version, copyright, analysis
 * progress, support link.
 *
 * The mark lives here rather than in the title bar. macOS draws the traffic
 * lights over the Overlay title bar at x:16, and anything placed at the left
 * of that bar collides with them — which is exactly what happened. The bottom
 * edge has no system furniture on it, so the mark can sit there unconditionally
 * and the title bar is freed for the controls that are actually used.
 */
export function StatusBar() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    // Read from the bundle rather than a literal, so it cannot drift from the
    // version the release workflow stamps in.
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  return (
    <footer className="flex h-8 shrink-0 items-center gap-2 border-t border-gray-700 bg-gray-900 px-3">
      <Logo size={13} />
      {version && <span className="mono text-[11px] text-gray-600">v{version}</span>}
      <span className="text-gray-700">·</span>
      <span className="text-[11px] text-gray-500">© 2026 Swendl</span>

      {/* Centred rather than pushed to one end: it is the only thing on this
          bar that changes, and a moving element next to the static byline
          reads as the byline moving. */}
      <span className="mx-auto flex items-center">
        <AnalysisStatus />
      </span>

      <button
        type="button"
        onClick={() => void openUrl(COFFEE).catch(() => {})}
        className="flex shrink-0 items-center gap-1.5 text-[11px] text-gray-500 transition-colors hover:text-keystone-lime"
      >
        <Coffee size={12} color="currentColor" variant="Linear" />
        Buy me a coffee
      </button>
    </footer>
  );
}

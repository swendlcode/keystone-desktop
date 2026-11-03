import { useCallback, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import { relaunch } from '@tauri-apps/plugin-process';
import { check as check_, type Update } from '@tauri-apps/plugin-updater';

/**
 * The running version, for About.
 *
 * Not part of `Settings`: it is the bundle's own number, not something the user
 * chose, and a settings field holding it would be a copy that can go stale
 * against the binary it is displayed in.
 */
export function useAppVersion() {
  return useQuery({
    queryKey: ['app-version'],
    queryFn: () => getVersion(),
    staleTime: Infinity,
  });
}

/**
 * Hand a URL to the system browser.
 *
 * A plain `target="_blank"` is unreliable inside the webview and on some
 * platforms opens inside the app window, which has no chrome to get back from.
 */
export function openExternal(url: string): void {
  openUrl(url).catch((e: unknown) => console.error('[settings] open failed:', e));
}

/**
 * CHECK FOR UPDATES, against the signed manifest.
 *
 * This goes to `tauri-plugin-updater`, which fetches and verifies a signed
 * `latest.json`. There is deliberately no HTTP client in the crate: a second,
 * unsigned path to "is there a newer build" would be a download with none of
 * the signature checking that makes the real one safe.
 *
 * A backend `check_for_update` used to sit beside this, hardcoded to answer
 * "no update available". It was registered, exposed through the service layer
 * and called by nothing — which is the only reason it never said "You are up
 * to date" to someone on a year-old build. Removed rather than left as a
 * trap.
 */
export function useUpdateCheck() {
  const [state, setState] = useState<UpdateCheck>({ status: 'idle' });

  const check = useCallback(() => {
    setState({ status: 'checking' });
    check_()
      .then((update) => {
        setState(
          update
            ? { status: 'available', version: update.version, update }
            : { status: 'current' },
        );
      })
      .catch((e: unknown) => setState({ status: 'failed', detail: String(e) }));
  }, []);

  /** Downloads, installs and relaunches. The restart is explicit because the
      user may be mid-set; nothing here happens without this call. */
  const install = useCallback(() => {
    if (state.status !== 'available') return;
    setState({ ...state, installing: true });
    state.update
      .downloadAndInstall()
      .then(() => relaunch())
      .catch((e: unknown) => setState({ status: 'failed', detail: String(e) }));
  }, [state]);

  return { state, check, install };
}

export type UpdateCheck =
  | { status: 'idle' }
  | { status: 'checking' }
  | { status: 'current' }
  | { status: 'available'; version: string; update: Update; installing?: boolean }
  | { status: 'failed'; detail: string };

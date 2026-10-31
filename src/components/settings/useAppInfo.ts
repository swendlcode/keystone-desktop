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
 * This goes to `tauri-plugin-updater` rather than to the backend's
 * `check_for_update`, which is a legacy version ping that always answers "no
 * update" by design — the crate has no HTTP client, because a second, unsigned
 * path to "is there a newer build" would be a download with none of the
 * signature checking that makes the real one safe. A button wired to the ping
 * would say "You are up to date" to a user on a year-old build.
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

/**
 * Transport shim for `@tauri-apps/api/core`.
 *
 * Vite aliases `@tauri-apps/api/core` to this module (see vite.config.ts), so
 * every `import { invoke, convertFileSrc } from '@tauri-apps/api/core'` — ours
 * and the plugins' — routes through here without a source change.
 *
 * Two consequences worth knowing before editing this file:
 *
 * 1. **Anything a plugin pulls from that specifier must be re-exported here**,
 *    or the plugin's import resolves to `undefined` and fails at runtime with
 *    no type error. `Resource` is the live example: `@tauri-apps/plugin-updater`
 *    extends it for its Update handle.
 * 2. The shim imports `@tauri-apps/api` (the package root), not
 *    `@tauri-apps/api/core`. The alias is an exact match on the `/core`
 *    specifier for that reason — importing `/core` here would resolve back to
 *    this file and recurse.
 *
 * Keystone has no browser mode: the analysis path is local by design and the
 * engine plan forbids a network hop. Outside the webview these calls fail
 * loudly rather than silently returning nothing, which is what a `vite dev`
 * tab opened by hand should do.
 */
import { core } from '@tauri-apps/api';

/** True inside the Tauri webview. False in a plain browser tab. */
export const isTauri =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const Channel = core.Channel;
export const Resource = core.Resource;

function noBridge(what: string): Error {
  return new Error(
    `${what} needs the Keystone desktop app — this page is running outside the Tauri webview.`,
  );
}

export function invoke<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri) return Promise.reject(noBridge(`Command "${cmd}"`));
  return core.invoke<T>(cmd, args);
}

/**
 * Turn an absolute file path into a URL the webview can load.
 *
 * Audio and cover art must go through this; a bare `file://` URL is blocked by
 * the webview. The asset protocol is enabled in tauri.conf.json.
 */
export function convertFileSrc(filePath: string, protocol = 'asset'): string {
  if (!isTauri) return '';
  return core.convertFileSrc(filePath, protocol);
}

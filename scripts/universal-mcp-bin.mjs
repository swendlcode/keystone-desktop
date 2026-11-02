#!/usr/bin/env node
/**
 * Lipo `keystone-mcp` for a universal macOS build. A no-op anywhere else.
 *
 * ## Why this exists, and why it is not `externalBin`
 *
 * tauri-cli ships every `[[bin]]` in the crate, so `keystone-mcp` reaches the
 * installers on its own. On a host build — including Windows — it is copied
 * straight out of `target/<profile>/` and nothing here is needed.
 *
 * A **universal** macOS build is the exception. Tauri lipos the *main* binary
 * into `target/universal-apple-darwin/<profile>/` and then copies every crate
 * bin out of that directory, but it does not lipo the extra ones, so they are
 * not there:
 *
 *   failed to bundle project: Failed to copy binary from
 *   "target/universal-apple-darwin/release/keystone-mcp": does not exist
 *
 * This builds both architectures and merges them into that directory, which is
 * the one thing missing.
 *
 * ## Do not solve this with `externalBin`
 *
 * It was, briefly. Declaring a crate bin as a sidecar installs the same file
 * twice and the Windows installer refuses it:
 *
 *   error LGHT0204 : ICE30: The target file 'keystone-mcp.exe' is installed
 *   in '[ProgramFiles64Folder]\Keystone\' by two different components on an
 *   LFN system: 'keystone_mcp.exe' and 'keystone_mcp'
 *
 * That is tauri-apps/tauri#14681. The declaration is both unnecessary and
 * harmful; this script is the whole of what is actually required.
 */

import { execFileSync } from 'node:child_process';
import { chmodSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const BIN = 'keystone-mcp';
const ARCHES = ['aarch64-apple-darwin', 'x86_64-apple-darwin'];

if (process.platform !== 'darwin') {
  console.log(`[universal-mcp-bin] not macOS; the bundler copies ${BIN} itself`);
  process.exit(0);
}

const tauriDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'src-tauri');
const release = process.argv.includes('--release');
const profile = release ? 'release' : 'debug';

const built = ARCHES.map((triple) => {
  execFileSync(
    'cargo',
    ['build', '--bin', BIN, ...(release ? ['--release'] : []), '--target', triple],
    { cwd: tauriDir, stdio: 'inherit' },
  );
  return join(tauriDir, 'target', triple, profile, BIN);
});

const outDir = join(tauriDir, 'target', 'universal-apple-darwin', profile);
mkdirSync(outDir, { recursive: true });
const merged = join(outDir, BIN);
execFileSync('lipo', ['-create', '-output', merged, ...built], { stdio: 'inherit' });
chmodSync(merged, 0o755);
console.log(`[universal-mcp-bin] ${merged}`);

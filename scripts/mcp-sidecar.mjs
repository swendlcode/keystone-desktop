#!/usr/bin/env node
/**
 * Build `keystone-mcp` and put it where Tauri's bundler looks for a sidecar.
 *
 * `externalBin` is resolved by the **build script**, not the bundler, so a
 * missing file fails `tauri dev` as hard as it fails `tauri build`:
 *
 *   resource path `binaries/keystone-mcp-aarch64-apple-darwin` doesn't exist
 *
 * That is why this runs from `beforeDevCommand` and `beforeBuildCommand`
 * rather than being a step someone has to remember. Forgetting it would break
 * the dev loop, which is the same way the second binary broke `cargo run`
 * before `default-run` was set.
 *
 * Naming is Tauri's: the configured path plus `-$TARGET_TRIPLE`, plus `.exe`
 * on Windows.
 *
 * ## Universal macOS
 *
 * `--target universal-apple-darwin` makes the build script ask for
 * `keystone-mcp-universal-apple-darwin`, and nothing builds that for us — it
 * is `lipo` over the two architectures. Sources disagree about whether the v2
 * CLI still merges sidecars itself, so on a macOS release this writes **all
 * three** names: both per-architecture files and the merged one. They are a
 * few megabytes and it removes the question from the release path, which is
 * not a good place to find out.
 *
 * ## Sharing cargo artifacts
 *
 * The profile and `--target` are chosen to match what Tauri is about to do,
 * so the library is compiled once rather than once per target layout:
 *
 * - macOS release: per-architecture with `--target`, which is what a
 *   universal build does anyway.
 * - everything else: no `--target`, which is what a host build does.
 */

import { execFileSync } from 'node:child_process';
import { chmodSync, copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const BIN = 'keystone-mcp';
const DARWIN_ARCHES = ['aarch64-apple-darwin', 'x86_64-apple-darwin'];

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const tauriDir = join(root, 'src-tauri');
const outDir = join(tauriDir, 'binaries');

const release = process.argv.includes('--release');
const profile = release ? 'release' : 'debug';

function hostTriple() {
  const out = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  const line = out.split('\n').find((l) => l.startsWith('host:'));
  if (!line) throw new Error('rustc did not report a host triple');
  return line.slice('host:'.length).trim();
}

const exeSuffix = (triple) => (triple.includes('windows') ? '.exe' : '');

/**
 * Build for one triple, or for the host when `target` is null.
 *
 * `named` is the triple the *filename* follows, which is not always `target`:
 * a host build passes `null` as the target so the artifacts land in the plain
 * `target/<profile>` directory and are shared with Tauri's own build, but the
 * file is still called `…-x86_64-pc-windows-msvc.exe`. Deriving the extension
 * from `process.platform` instead is what broke the Windows release job —
 * `'win32'.includes('windows')` is false, so it looked for a source file with
 * no `.exe` on it.
 */
function build(target, named) {
  const args = ['build', '--bin', BIN];
  if (release) args.push('--release');
  if (target) args.push('--target', target);
  execFileSync('cargo', args, { cwd: tauriDir, stdio: 'inherit' });

  const dir = target
    ? join(tauriDir, 'target', target, profile)
    : join(tauriDir, 'target', profile);
  return join(dir, BIN + exeSuffix(named));
}

function place(from, triple) {
  const to = join(outDir, `${BIN}-${triple}${exeSuffix(triple)}`);
  copyFileSync(from, to);
  chmodSync(to, 0o755);
  console.log(`[mcp-sidecar] ${to}`);
  return to;
}

mkdirSync(outDir, { recursive: true });

if (release && process.platform === 'darwin') {
  const built = [];
  for (const triple of DARWIN_ARCHES) {
    try {
      built.push([triple, build(triple, triple)]);
    } catch (e) {
      // A machine without the cross target installed can still produce a
      // single-architecture build. The universal name is then missing, and
      // Tauri's own "resource path doesn't exist" is a clearer report of
      // that than anything invented here.
      console.warn(`[mcp-sidecar] could not build ${triple}: ${e.message}`);
    }
  }
  for (const [triple, path] of built) place(path, triple);

  if (built.length === DARWIN_ARCHES.length) {
    const universal = join(outDir, `${BIN}-universal-apple-darwin`);
    execFileSync('lipo', ['-create', '-output', universal, built[0][1], built[1][1]], {
      stdio: 'inherit',
    });
    chmodSync(universal, 0o755);
    console.log(`[mcp-sidecar] ${universal}`);

    // And where the bundler actually reads it from.
    //
    // The build script resolves `externalBin` against `binaries/`, which is
    // why the name above is needed and why a missing file fails compilation.
    // The *bundler* does not: for `--target universal-apple-darwin` it copies
    // the sidecar out of the target directory, and nothing puts it there —
    //
    //   failed to bundle project: Failed to copy binary from
    //   "target/universal-apple-darwin/release/keystone-mcp": does not exist
    //
    // which is how a release job got all the way through two architectures
    // before falling over. Both locations, so both stages are satisfied.
    const targetDir = join(tauriDir, 'target', 'universal-apple-darwin', profile);
    mkdirSync(targetDir, { recursive: true });
    const resolved = join(targetDir, BIN);
    copyFileSync(universal, resolved);
    chmodSync(resolved, 0o755);
    console.log(`[mcp-sidecar] ${resolved}`);
  } else {
    console.warn(
      '[mcp-sidecar] only one architecture built, so there is no universal sidecar; ' +
        'a --target universal-apple-darwin build will refuse to start',
    );
  }
} else {
  const host = hostTriple();
  place(build(null, host), host);
}

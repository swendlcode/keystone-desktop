import buildMeta from '../data/build.json';

/**
 * Which build this is.
 *
 * The version number says what changed; the build number says which copy you
 * are running. They are not the same question, and only the first could be
 * answered from anything on screen: several builds of one version go on and
 * off this machine while a single bug is chased, and every one of them calls
 * itself the same thing in About and along the status bar.
 *
 * `scripts/install-local.sh` increments this before every build, so the number
 * is a count of builds that exist rather than something anyone has to
 * remember to bump.
 */
export const BUILD_NUMBER: number = buildMeta.build;

/** `0.1.4 (12)` — the version with the build it came from. */
export function versionLabel(version: string): string {
  if (!version) return '';
  return `${version} (${BUILD_NUMBER})`;
}

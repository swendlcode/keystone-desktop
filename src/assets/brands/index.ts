/**
 * Brand marks for the export targets, if they have been supplied.
 *
 * Resolved at build time from whatever is in this folder. The files are
 * other companies' trademarks, so they are not in the repository: a clean
 * checkout finds nothing here and `TargetMark` falls back to its lettered
 * badges, which is a complete build rather than a broken one.
 *
 * `eager` because there are at most three of them and a lazy import would
 * make a 20px icon flash in after the row has rendered.
 */
const FILES = import.meta.glob('./*.{svg,png}', {
  eager: true,
  query: '?url',
  import: 'default',
}) as Record<string, string>;

/** `rekordbox` -> its asset URL, when one was dropped in. */
export const BRAND_MARKS: Record<string, string> = Object.fromEntries(
  Object.entries(FILES).map(([path, url]) => [
    path.replace(/^\.\//, '').replace(/\.(svg|png)$/, ''),
    url,
  ]),
);

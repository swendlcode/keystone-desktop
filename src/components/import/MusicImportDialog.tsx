import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { open } from '@tauri-apps/plugin-dialog';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Button } from '../ui/Button';
import { Checkbox } from '../ui/Checkbox';
import { ScrollArea } from '../ui/ScrollArea';
import { MusicSquare } from '../ui/icons';
import { keystoneService } from '../../services/keystoneService';
import { trackKeys } from '../../hooks/useLibrary';
import { formatCount } from '../../utils/formatters';

/**
 * Import playlists from the Music app, or from a playlist file.
 *
 * The counts are files that exist on disk right now, not the playlist's
 * length — see `MusicPlaylist.trackCount`. A subscription playlist of 173
 * streams and four purchases imports four tracks, and the only honest number
 * to put next to it is four.
 *
 * Nothing is written back to Music, and the playlist is not recreated as a
 * collection: this adds the files to the library, which is what the button it
 * hangs off says. Filing them is the user's next gesture, against collections
 * they already have.
 */
export function MusicImportDialog({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const [chosen, setChosen] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<number | null>(null);
  /** An exported XML the user picked, instead of the default location. */
  const [override, setOverride] = useState<string | null>(null);

  const library = useQuery({
    queryKey: ['music-library', override],
    queryFn: () => keystoneService.getMusicLibrary(override),
    // The XML is written when Music feels like it, so a cached answer from
    // ten minutes ago is worth less than the round trip to re-read it.
    staleTime: 0,
    gcTime: 0,
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !busy) onClose();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [busy, onClose]);

  const playlists = library.data?.playlists ?? [];
  const xmlPath = library.data?.xmlPath ?? null;

  const refreshLibrary = () => {
    void qc.invalidateQueries({ queryKey: trackKeys.all });
    void qc.invalidateQueries({ queryKey: ['library-folders'] });
  };

  const runImport = async () => {
    if (!xmlPath || chosen.length === 0) return;
    setBusy(true);
    try {
      setDone(await keystoneService.importMusicPlaylists(xmlPath, chosen));
      refreshLibrary();
    } catch (e) {
      console.error('[import] playlist import failed:', e);
      setDone(0);
    } finally {
      setBusy(false);
    }
  };

  /**
   * The route that does not depend on Apple publishing anything.
   *
   * Every DJ tool exports an `.m3u`, and Music itself can export one from
   * File > Library > Export Playlist — which is also the answer for anyone
   * who would rather not leave XML sharing switched on.
   *
   * A library `.xml` is a container of playlists rather than a playlist, so
   * choosing one re-points the list above at it and waits. Importing it
   * directly would mean importing against whatever happened to be ticked for
   * a different library, which on a first visit is nothing at all.
   */
  const pickPlaylistFile = async () => {
    const picked = await open({
      multiple: false,
      filters: [
        { name: 'Playlist', extensions: ['m3u', 'm3u8', 'pls', 'xml'] },
      ],
    });
    if (typeof picked !== 'string') return;

    if (picked.toLowerCase().endsWith('.xml')) {
      setChosen([]);
      setDone(null);
      setOverride(picked);
      return;
    }

    setBusy(true);
    try {
      setDone(await keystoneService.importPlaylistFile(picked));
      refreshLibrary();
    } catch (e) {
      console.error('[import] playlist file failed:', e);
      setDone(0);
    } finally {
      setBusy(false);
    }
  };

  const toggle = (name: string) =>
    setChosen((prev) =>
      prev.includes(name) ? prev.filter((n) => n !== name) : [...prev, name],
    );

  const total = playlists
    .filter((p) => chosen.includes(p.name))
    .reduce((sum, p) => sum + p.trackCount, 0);

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Import playlists"
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 p-4"
    >
      <div className="flex max-h-[80vh] w-full max-w-lg flex-col rounded-xl border border-gray-700 bg-gray-900 p-5 shadow-2xl">
        <div className="text-center">
          <MusicSquare
            size={28}
            color="currentColor"
            variant="Bold"
            className="mx-auto text-keystone-lime"
          />
          <h3 className="mt-2 text-base font-semibold text-keystone-white">
            Import playlists
          </h3>
          <p className="mt-1 text-xs text-gray-400">
            {done != null
              ? `${formatCount(done)} ${done === 1 ? 'track' : 'tracks'} queued for analysis.`
              : 'Straight from the Music app, or from a playlist file.'}
          </p>
        </div>

        <div className="mt-4 min-h-0 flex-1 overflow-hidden rounded-lg border border-gray-700">
          {library.isLoading ? (
            <p className="p-5 text-center text-xs text-gray-500">
              Reading the Music library…
            </p>
          ) : !xmlPath ? (
            <NoLibrary />
          ) : playlists.length === 0 ? (
            <p className="p-5 text-center text-xs text-gray-500">
              No playlists in this library hold a local file.
            </p>
          ) : (
            <>
              <div className="flex items-center justify-between border-b border-gray-700 bg-gray-800 px-3 py-2">
                <span className="label">Playlist</span>
                <span className="label">Tracks</span>
              </div>
              <ScrollArea className="max-h-64">
                {playlists.map((p) => (
                  <div
                    key={p.name}
                    className="flex items-center gap-2 border-b border-gray-800/60 px-3 py-2 text-xs hover:bg-gray-800/60"
                  >
                    {/* Checkbox renders its own <label>, so this row must not
                        be one: nested labels are invalid and the inner input
                        then takes two clicks to toggle once. */}
                    <Checkbox
                      checked={chosen.includes(p.name)}
                      onChange={() => toggle(p.name)}
                      label={p.name}
                      className="min-w-0 flex-1"
                    />
                    <span className="mono tnum shrink-0 text-gray-400">
                      {formatCount(p.trackCount)}
                    </span>
                  </div>
                ))}
              </ScrollArea>
            </>
          )}
        </div>

        <div className="mt-4 flex items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            disabled={busy}
            onClick={pickPlaylistFile}
          >
            Choose a file…
          </Button>
          <span className="mono ml-auto text-[11px] text-gray-500">
            {chosen.length > 0 && `${formatCount(total)} selected`}
          </span>
          <Button variant="ghost" size="sm" disabled={busy} onClick={onClose}>
            {done == null ? 'Cancel' : 'Close'}
          </Button>
          <Button
            variant="primary"
            size="sm"
            disabled={busy || !xmlPath || chosen.length === 0}
            onClick={() => void runImport()}
          >
            {busy ? 'Importing…' : 'Import'}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

/**
 * What to say when there is no XML to read.
 *
 * Which is the normal case: Catalina moved the library to a binary file and
 * made the XML opt-in. This is a setting the user can turn on, not a failure,
 * so it names the setting — and offers the route that works without it.
 */
function NoLibrary() {
  return (
    <div className="p-5 text-center">
      <p className="text-xs text-keystone-white">
        The Music app is not publishing a library file.
      </p>
      <p className="mt-2 text-xs leading-relaxed text-gray-400">
        Turn on <span className="text-gray-300">Music → Settings → Advanced
        → Share Library XML with other applications</span>, then reopen this
        window. Or export a playlist from{' '}
        <span className="text-gray-300">File → Library → Export Playlist</span>{' '}
        and choose the file below.
      </p>
    </div>
  );
}

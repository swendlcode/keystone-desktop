import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { Button } from '../ui/Button';
import { DocumentUpload } from '../ui/icons';

/**
 * What ADD TRACKS asks before it opens anything.
 *
 * The button used to go straight to a folder picker, which quietly made
 * "point Keystone at a folder" the only way in. A DJ's library is just as
 * likely to be a set of playlists they already maintain somewhere else, and
 * there is no folder on disk that corresponds to one. One extra click buys
 * both routes and, more usefully, says out loud that dropping files on the
 * window works too — a feature nothing else in the UI mentions.
 */
export function AddTracksDialog({
  onFolder,
  onPlaylist,
  onCancel,
}: {
  onFolder: () => void;
  onPlaylist: () => void;
  onCancel: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [onCancel]);

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Add tracks"
      onMouseDown={(e) => e.target === e.currentTarget && onCancel()}
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 p-4"
    >
      <div className="w-full max-w-md rounded-xl border border-gray-700 bg-gray-900 px-6 py-7 text-center shadow-2xl">
        <DocumentUpload
          size={34}
          color="currentColor"
          variant="Bold"
          className="mx-auto text-keystone-lime"
        />
        <h3 className="mt-3 text-base font-semibold text-keystone-white">
          Add tracks to analyse
        </h3>
        <p className="mt-1.5 text-xs text-gray-400">
          You can also drop files or folders straight onto the window.
        </p>

        <div className="mt-6 flex items-center justify-center gap-2">
          <Button variant="ghost" size="sm" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="primary" size="sm" onClick={onFolder}>
            Files or folders
          </Button>
          <Button variant="primary" size="sm" onClick={onPlaylist}>
            Playlist
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

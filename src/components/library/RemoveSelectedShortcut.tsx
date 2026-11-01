import { useEffect, useState } from 'react';
import { ConfirmDialog } from '../settings/ConfirmDialog';
import { useRemoveTracks } from '../../hooks/useLibrary';
import { useLibraryStore } from '../../stores/libraryStore';
import { formatCount } from '../../utils/formatters';

/**
 * Backspace or Delete removes the selected tracks, behind a confirmation.
 *
 * Mounted once from App rather than per table, so it works in the library and
 * in the tag view from one listener and one dialog.
 *
 * It asks every time, with no "don't show this again". The rest of Keystone
 * treats a destructive action as worth a sentence, and this is the one that
 * is reachable by leaning on a key — the distance between selecting 4,000
 * rows and emptying them from the library is otherwise a single keystroke
 * with no gesture in between.
 *
 * The files are never touched; `remove_tracks_from_library` defaults to
 * leaving them on disk, and the dialog says so because a key labelled Delete
 * invites exactly the wrong assumption.
 */
export function RemoveSelectedShortcut() {
  const [pending, setPending] = useState<string[] | null>(null);
  const remove = useRemoveTracks();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Backspace' && e.key !== 'Delete') return;

      // Never steal the key from a field. The tag editor is a grid of them
      // and Backspace there means backspace.
      const el = e.target as HTMLElement | null;
      if (
        el &&
        (el.tagName === 'INPUT' ||
          el.tagName === 'TEXTAREA' ||
          el.isContentEditable)
      ) {
        return;
      }

      const ids = [...useLibraryStore.getState().selectedIds];
      if (ids.length === 0) return;
      e.preventDefault();
      setPending(ids);
    };

    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  if (!pending) return null;

  const many = pending.length > 1;

  return (
    <ConfirmDialog
      title={
        many
          ? `Remove ${formatCount(pending.length)} tracks?`
          : 'Remove this track?'
      }
      body={`${many ? 'They leave' : 'It leaves'} Keystone. ${
        many ? 'The files' : 'The file'
      } on disk ${many ? 'are' : 'is'} untouched.`}
      confirmLabel="Remove"
      destructive
      busy={remove.isPending}
      onCancel={() => setPending(null)}
      onConfirm={() => {
        remove.mutate(pending);
        setPending(null);
      }}
    />
  );
}

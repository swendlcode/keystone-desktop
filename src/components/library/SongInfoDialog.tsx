import { useEffect, useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import { Button } from '../ui/Button';
import { Input } from '../ui/Input';
import { KeyBadge } from '../ui/KeyBadge';
import { Next, Previous, Tag } from '../ui/icons';
import { keystoneService } from '../../services/keystoneService';
import { useSetTrackTempo, useTrack } from '../../hooks/useLibrary';
import { useWriteTags } from '../tags/useTagEditing';
import { useEffectiveSettings } from '../../hooks/useSettings';
import { useUiStore } from '../../stores/uiStore';
import type { TagField } from '../../types';

/** The tag fields this panel edits, in the order it shows them. */
const FIELDS: Array<{ field: TagField; label: string }> = [
  { field: 'artist', label: 'Tag artist' },
  { field: 'title', label: 'Tag title' },
  { field: 'album', label: 'Tag album' },
  { field: 'genre', label: 'Tag genre' },
  { field: 'comment', label: 'Tag comment' },
];

/**
 * Everything about one track, in one place, editable.
 *
 * The context menu has offered Song Info since it was written and nothing
 * rendered it — `openInfo` set a store field no component read, so the row
 * was dead. This is that panel.
 *
 * Edits are **buffered and applied on OK**, unlike the table cells which
 * commit as you leave them. Every tag write here is a rewrite of the file
 * through the safe path, and a panel with six fields that wrote on each blur
 * would rewrite the file six times for one visit — and give no way to change
 * your mind halfway.
 */
export function SongInfoDialog() {
  const trackId = useUiStore((s) => s.infoTrackId);
  const siblings = useUiStore((s) => s.infoSiblings);
  const openInfo = useUiStore((s) => s.openInfo);
  const close = useUiStore((s) => s.closeInfo);

  const { data: track } = useTrack(trackId);
  const settings = useEffectiveSettings();
  const writeTags = useWriteTags();
  const setTempo = useSetTrackTempo();

  const [draft, setDraft] = useState<Record<string, string>>({});
  const [tempo, setTempoDraft] = useState('');

  // A different track is a different set of values; a draft carried across
  // would write one track's artist onto another.
  useEffect(() => {
    if (!track) return;
    setDraft({
      artist: track.artist ?? '',
      title: track.title ?? '',
      album: track.album ?? '',
      genre: track.genre ?? '',
      comment: track.comment ?? '',
    });
    setTempoDraft(track.tempoDisplay == null ? '' : track.tempoDisplay.toFixed(2));
  }, [track]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [close]);

  const at = useMemo(
    () => (trackId ? siblings.indexOf(trackId) : -1),
    [siblings, trackId],
  );
  const step = (by: number) => {
    const next = siblings[at + by];
    if (next) openInfo(next, siblings);
  };

  if (!trackId || !track) return null;

  const busy = writeTags.isPending || setTempo.isPending;

  const apply = () => {
    const changed: Array<[TagField, string | null]> = [];
    for (const { field } of FIELDS) {
      const was = (track[field as keyof typeof track] as string | null) ?? '';
      const now = draft[field] ?? '';
      // An emptied field means "remove this tag", which is not the same as
      // writing an empty string into it.
      if (now !== was) changed.push([field, now.trim() === '' ? null : now]);
    }
    if (changed.length > 0) {
      writeTags.mutate({ trackId: track.id, edit: { fields: changed } });
    }

    const bpm = Number(tempo);
    if (
      Number.isFinite(bpm) &&
      bpm > 0 &&
      Math.abs(bpm - (track.tempoDisplay ?? 0)) > 0.005
    ) {
      setTempo.mutate({ id: track.id, bpm });
    }
    close();
  };

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Song info"
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 p-4"
    >
      <div className="flex max-h-[86vh] w-full max-w-lg flex-col rounded-xl border border-gray-700 bg-gray-900 shadow-2xl">
        <div className="flex items-center justify-center gap-2 border-b border-gray-700 py-4">
          <Tag size={18} color="currentColor" variant="Bold" className="text-keystone-lime" />
          <h3 className="text-sm font-semibold text-keystone-white">Song info</h3>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
          <div className="flex gap-4">
            <Readout label="Main key">
              <KeyBadge
                camelot={track.camelot}
                notation={settings?.keyNotation}
                pad={settings?.padCamelotNumbers}
              />
            </Readout>
            <Readout label="Tempo">
              <Input
                numeric
                value={tempo}
                className="h-8 w-28"
                onChange={(e) => setTempoDraft(e.target.value)}
              />
            </Readout>
            <Readout label="Energy">
              <span className="mono tnum text-sm text-keystone-white">
                {track.energy ?? '—'}
              </span>
            </Readout>
          </div>

          <p className="label mt-5">File name</p>
          <p className="mono mt-1 break-all text-xs text-keystone-white">
            {track.filename}
          </p>
          {/* The path reveals rather than edits. Moving a file is a rename,
              which has its own dry run and its own undo, and burying it in a
              text field here would be the one irreversible thing on the
              panel with the least ceremony around it. */}
          <button
            type="button"
            onClick={() => void keystoneService.revealInFinder(track.path).catch(() => {})}
            title="Reveal in Finder"
            className="mono mt-1 break-all text-left text-[11px] text-keystone-lime hover:underline"
          >
            {track.path}
          </button>

          <div className="mt-5 grid grid-cols-2 gap-x-4 gap-y-4">
            {FIELDS.map(({ field, label }) => (
              <label key={field} className={field === 'comment' ? 'col-span-2' : ''}>
                <span className="label">{label}</span>
                <Input
                  value={draft[field] ?? ''}
                  className="mt-1 h-8"
                  onChange={(e) =>
                    setDraft((d) => ({ ...d, [field]: e.target.value }))
                  }
                />
              </label>
            ))}
          </div>
        </div>

        <div className="flex items-center gap-2 border-t border-gray-700 px-6 py-4">
          <Button
            variant="secondary"
            size="sm"
            disabled={at <= 0}
            onClick={() => step(-1)}
            icon={<Previous size={12} color="currentColor" variant="Linear" />}
          >
            Previous
          </Button>
          <Button
            variant="secondary"
            size="sm"
            disabled={at < 0 || at >= siblings.length - 1}
            onClick={() => step(1)}
            icon={<Next size={12} color="currentColor" variant="Linear" />}
          >
            Next
          </Button>
          {siblings.length > 1 && (
            <span className="mono text-[11px] text-gray-500">
              {at + 1} / {siblings.length}
            </span>
          )}

          <span className="ml-auto flex items-center gap-2">
            <Button variant="ghost" size="sm" disabled={busy} onClick={close}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" disabled={busy} onClick={apply}>
              {busy ? 'Saving...' : 'OK'}
            </Button>
          </span>
        </div>
      </div>
    </div>,
    document.body,
  );
}

function Readout({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <span className="flex flex-col gap-1">
      <span className="label">{label}</span>
      {children}
    </span>
  );
}

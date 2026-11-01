import { useCallback, useMemo, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Button } from '../ui/Button';
import { Checkbox } from '../ui/Checkbox';
import { Input } from '../ui/Input';
import { Magicpen } from '../ui/icons';
import { useFileWriteRun } from '../settings/useFileWriteRun';
import { TagRunDialogs } from '../settings/TagRunDialogs';
import { keystoneService } from '../../services/keystoneService';
import { trackKeys } from '../../hooks/useLibrary';
import { useLibraryStore } from '../../stores/libraryStore';
import { REPLACE_FIELDS, type TagField, type TagWriteReport } from '../../types';

/** `initialKey` is the Key column in SPEC.md §2's field list. */
const FIELD_LABELS: Partial<Record<TagField, string>> = {
  artist: 'Artist',
  title: 'Title',
  initialKey: 'Key',
  genre: 'Genre',
  album: 'Album',
  grouping: 'Grouping',
  comment: 'Comment',
};

/**
 * Global Replace, across the fields SPEC.md §2 names.
 *
 * It rewrites real files in bulk, so it is dry-run-first and undoable: the
 * dialog states how many files would change before one is opened, and the
 * banner afterwards carries Undo. The engine plan is blunt about why this
 * particular feature gets the full treatment — a careless bulk replace is the
 * one operation in the app that can delete every DJ tag in a library in a
 * single click.
 */
export function GlobalReplacePanel() {
  const qc = useQueryClient();
  const selectedIds = useLibraryStore((s) => s.selectedIds);
  const ids = useMemo(() => [...selectedIds], [selectedIds]);
  const [find, setFind] = useState('');
  const [replace, setReplace] = useState('');
  const [fields, setFields] = useState<TagField[]>(['artist', 'title']);

  const dryRun = useCallback(
    () => keystoneService.globalReplace({ trackIds: ids, find, replace, fields, dryRun: true }),
    [fields, find, ids, replace],
  );
  const apply = useCallback(
    () => keystoneService.globalReplace({ trackIds: ids, find, replace, fields, dryRun: false }),
    [fields, find, ids, replace],
  );
  const onDone = useCallback(() => {
    qc.invalidateQueries({ queryKey: trackKeys.all });
  }, [qc]);

  const run = useFileWriteRun<TagWriteReport, TagWriteReport>({ dryRun, apply, onDone });
  const ready = find.length > 0 && fields.length > 0 && ids.length > 0;

  const toggle = (f: TagField) =>
    setFields((prev) => (prev.includes(f) ? prev.filter((x) => x !== f) : [...prev, f]));

  return (
    <div className="flex flex-col gap-3 border-b border-gray-700 p-3">
      <ReplaceInputs
        find={find}
        replace={replace}
        ready={ready}
        busy={run.busy}
        onFind={setFind}
        onReplace={setReplace}
        onRun={run.start}
      />

      <FieldChecks fields={fields} onToggle={toggle} />

      <p className="text-[12px] text-gray-500">
        {ids.length === 0 ? 'Select tracks first.' : `${ids.length} selected.`}
      </p>
      {run.error && <p className="text-[12px] text-amber-400">{run.error}</p>}

      <TagRunDialogs
        run={run}
        trackIds={ids}
        title="Replace in these files?"
        verb="Change"
      />
    </div>
  );
}

function ReplaceInputs({
  find,
  replace,
  ready,
  busy,
  onFind,
  onReplace,
  onRun,
}: {
  find: string;
  replace: string;
  ready: boolean;
  busy: boolean;
  onFind: (v: string) => void;
  onReplace: (v: string) => void;
  onRun: () => void;
}) {
  return (
    <div className="flex items-end gap-2">
      <label className="min-w-0 flex-1">
        <span className="label mb-1 block">Replace</span>
        <Input value={find} onChange={(e) => onFind(e.target.value)} placeholder="Find" />
      </label>
      <label className="min-w-0 flex-1">
        <span className="label mb-1 block">With</span>
        <Input
          value={replace}
          onChange={(e) => onReplace(e.target.value)}
          placeholder="Leave empty to delete"
        />
      </label>
      <Button
        variant="primary"
        disabled={!ready || busy}
        onClick={onRun}
        icon={<Magicpen size={14} color="currentColor" variant="Linear" />}
      >
        REPLACE ALL
      </Button>
    </div>
  );
}

function FieldChecks({
  fields,
  onToggle,
}: {
  fields: TagField[];
  onToggle: (field: TagField) => void;
}) {
  return (
    <div className="flex flex-wrap gap-x-5 gap-y-1.5">
      {REPLACE_FIELDS.map((f) => (
        <Checkbox
          key={f}
          checked={fields.includes(f)}
          onChange={() => onToggle(f)}
          label={FIELD_LABELS[f] ?? f}
        />
      ))}
    </div>
  );
}

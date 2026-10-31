import { useCallback, useMemo } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Button } from '../ui/Button';
import { Tag } from '../ui/icons';
import { keystoneService } from '../../services/keystoneService';
import { trackKeys } from '../../hooks/useLibrary';
import { useLibraryStore } from '../../stores/libraryStore';
import { SettingNote } from './SettingSection';
import { TagRunDialogs } from './TagRunDialogs';
import { useFileWriteRun } from './useFileWriteRun';
import type { TagWriteReport } from '../../types';

/**
 * RE-TAG EXISTING FILES (SPEC.md §3 Update Tags).
 *
 * Library-wide when nothing is selected, which is what the label says, and
 * scoped to the selection when there is one. Two commands rather than one
 * because the library-wide pass never materialises a list of every id — on a
 * 20,000-track library that round trip is the slow part, and the backend
 * already knows how to walk its own table.
 *
 * Dry run first, always: the dialog shows the count the run would change before
 * a single file is opened, and a count of zero cannot be confirmed. That is the
 * engine plan's safety rule rather than a nicety — this rewrites tags in place
 * across a library that may be the user's whole working set.
 */
export function RetagAction({ disabled }: { disabled: boolean }) {
  const qc = useQueryClient();
  const selectedIds = useLibraryStore((s) => s.selectedIds);
  const ids = useMemo(() => [...selectedIds], [selectedIds]);
  const wholeLibrary = ids.length === 0;

  const dryRun = useCallback(
    () =>
      wholeLibrary
        ? keystoneService.retagExistingFiles(true)
        : keystoneService.retagTracks(ids, true),
    [ids, wholeLibrary],
  );
  const apply = useCallback(
    () =>
      wholeLibrary
        ? keystoneService.retagExistingFiles(false)
        : keystoneService.retagTracks(ids, false),
    [ids, wholeLibrary],
  );
  const onDone = useCallback(() => {
    qc.invalidateQueries({ queryKey: trackKeys.all });
  }, [qc]);

  const run = useFileWriteRun<TagWriteReport, TagWriteReport>({ dryRun, apply, onDone });

  return (
    <div className="flex flex-col gap-2">
      <Button
        variant="primary"
        size="sm"
        className="self-start"
        disabled={disabled || run.busy}
        onClick={run.start}
        icon={<Tag size={13} color="currentColor" variant="Linear" />}
      >
        RE-TAG EXISTING FILES
      </Button>

      <SettingNote>
        {wholeLibrary
          ? 'Nothing selected, so this covers the whole library. Keystone backs up every file’s tags before writing.'
          : `${ids.length} selected. Keystone backs up each file’s tags before writing.`}
      </SettingNote>

      {run.error && <SettingNote tone="warn">{run.error}</SettingNote>}

      <TagRunDialogs
        run={run}
        trackIds={ids}
        title={wholeLibrary ? 'Re-tag the whole library?' : 'Re-tag these files?'}
        verb="Write"
      />
    </div>
  );
}

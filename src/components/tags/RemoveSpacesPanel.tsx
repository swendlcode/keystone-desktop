import { useCallback, useMemo } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Button } from '../ui/Button';
import { Scissor } from '../ui/icons';
import { useFileWriteRun } from '../settings/useFileWriteRun';
import { TagRunDialogs } from '../settings/TagRunDialogs';
import { keystoneService } from '../../services/keystoneService';
import { trackKeys } from '../../hooks/useLibrary';
import { useLibraryStore } from '../../stores/libraryStore';
import type { TagWriteReport } from '../../types';

/**
 * Remove Extra Spaces.
 *
 * Same plan-confirm-undo path as Global Replace, for the same reason: it is a
 * bulk write to the user's files. It looks harmless and is not — a double space
 * inside a title is sometimes deliberate, and on a library of thousands nobody
 * can eyeball the result afterwards.
 */
export function RemoveSpacesPanel() {
  const qc = useQueryClient();
  const selectedIds = useLibraryStore((s) => s.selectedIds);
  const ids = useMemo(() => [...selectedIds], [selectedIds]);

  const dryRun = useCallback(() => keystoneService.removeExtraSpaces(ids, true), [ids]);
  const apply = useCallback(() => keystoneService.removeExtraSpaces(ids, false), [ids]);
  const onDone = useCallback(() => {
    qc.invalidateQueries({ queryKey: trackKeys.all });
  }, [qc]);

  const run = useFileWriteRun<TagWriteReport, TagWriteReport>({ dryRun, apply, onDone });

  return (
    <div className="flex flex-col gap-3 border-b border-gray-700 p-3">
      <div className="flex items-center justify-between gap-3">
        <p className="text-xs text-gray-300">
          Collapses double spaces and trims the ends of every text tag.
        </p>
        <Button
          variant="primary"
          disabled={run.busy || ids.length === 0}
          onClick={run.start}
          icon={<Scissor size={14} color="currentColor" variant="Linear" />}
        >
          REMOVE EXTRA SPACES
        </Button>
      </div>

      <p className="text-[12px] text-gray-500">
        {ids.length === 0 ? 'Select tracks first.' : `${ids.length} selected.`}
      </p>
      {run.error && <p className="text-[12px] text-amber-400">{run.error}</p>}

      <TagRunDialogs run={run} trackIds={ids} title="Tidy these files?" verb="Tidy" />
    </div>
  );
}

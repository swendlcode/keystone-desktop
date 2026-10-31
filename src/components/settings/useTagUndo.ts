import { useCallback, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../../services/keystoneService';
import { trackKeys } from '../../hooks/useLibrary';

export interface TagUndo {
  /** Restores the newest backup of every track the run touched. */
  undo: (trackIds: string[]) => void;
  busy: boolean;
  /** How many files were put back, or null before an undo has finished. */
  restored: number | null;
  error: string | null;
}

const BATCH = 8;

/**
 * Undo for a bulk tag write.
 *
 * A bulk replace rewrites real files, so it has to be reversible. Every write
 * goes through `tags::backup` first, and this walks those backups: newest entry
 * per track, restored through the same safe write path.
 *
 * It restores the newest backup of each id it is *given*, which is the id list
 * the run was started with — not a list the report hands back, because
 * `TagWriteReport` carries counts and failures but no ids. A `restore_tag_run
 * (runId)` command would be exact and is the right shape later; this is correct
 * for the normal case, where the last thing that touched those files was the
 * run being undone.
 *
 * Batched rather than fired all at once: an undo across 5,000 tracks is 5,000
 * file copies, and saturating the pool makes the window unresponsive for the
 * whole of it.
 */
export function useTagUndo(): TagUndo {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [restored, setRestored] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const undo = useCallback(
    (trackIds: string[]) => {
      setBusy(true);
      setError(null);
      restoreNewest(trackIds)
        .then((n) => {
          setRestored(n);
          qc.invalidateQueries({ queryKey: trackKeys.all });
        })
        .catch((e: unknown) => setError(String(e)))
        .finally(() => setBusy(false));
    },
    [qc],
  );

  return { undo, busy, restored, error };
}

async function restoreNewest(trackIds: string[]): Promise<number> {
  let done = 0;
  for (let i = 0; i < trackIds.length; i += BATCH) {
    const slice = trackIds.slice(i, i + BATCH);
    const results = await Promise.all(slice.map(restoreOne));
    done += results.filter(Boolean).length;
  }
  return done;
}

async function restoreOne(trackId: string): Promise<boolean> {
  const backups = await keystoneService.listTagBackups(trackId);
  if (!backups.length) return false;
  const newest = backups.reduce((a, b) => (b.writtenAt > a.writtenAt ? b : a));
  await keystoneService.restoreTagBackup(trackId, newest.id);
  return true;
}

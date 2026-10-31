import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { keystoneService } from '../../services/keystoneService';
import type { ExportTarget, Settings } from '../../types';

export const exportKeys = {
  targets: ['export-targets'] as const,
  backups: (target: ExportTarget) => ['export-backups', target] as const,
};

/** Which settings flag turns each target on. */
export const TARGET_FLAG: Record<ExportTarget, keyof Settings> = {
  serato: 'exportSerato',
  traktor: 'exportTraktor',
  rekordbox: 'exportRekordbox',
};

export const TARGET_LABEL: Record<ExportTarget, string> = {
  serato: 'Serato',
  traktor: 'Traktor',
  rekordbox: 'rekordbox',
};

/** SPEC.md's order. rekordbox first: it is the one that cannot damage anything. */
export const TARGET_ORDER: ExportTarget[] = ['serato', 'traktor', 'rekordbox'];

/**
 * Detection for the three targets.
 *
 * Refetched on mount rather than cached for long: a target's `running` flag
 * decides whether the export is refused, and the user's answer to "Traktor is
 * open" is to close Traktor and come straight back.
 */
export function useExportTargets() {
  return useQuery({
    queryKey: exportKeys.targets,
    queryFn: () => keystoneService.detectExportTargets(),
    staleTime: 0,
  });
}

export function useSetExportPath() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ target, path }: { target: ExportTarget; path: string | null }) =>
      keystoneService.setExportPath(target, path),
    onSuccess: () => qc.invalidateQueries({ queryKey: exportKeys.targets }),
  });
}

/** The dated copies taken before the first write to a target. */
export function useExportBackups(target: ExportTarget, enabled: boolean) {
  return useQuery({
    queryKey: exportKeys.backups(target),
    queryFn: () => keystoneService.listExportBackups(target),
    enabled,
  });
}

export function useRestoreExportBackup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ target, path }: { target: ExportTarget; path: string }) =>
      keystoneService.restoreExportBackup(target, path),
    onSuccess: (_r, { target }) =>
      qc.invalidateQueries({ queryKey: exportKeys.backups(target) }),
  });
}

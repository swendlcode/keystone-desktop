import { ArrowRight2 } from '../ui/icons';
import { SettingNote } from './SettingSection';
import type { RenamePlan } from '../../types';

const SHOWN = 8;

/**
 * What a rename would do, before anything moves.
 *
 * Renaming is the only irreversible operation in Keystone, and one setting runs
 * it automatically after every analysis. So the plan is shown as rows of
 * from -> to, and the items the planner refused are shown with their reason:
 * a skipped file is information, not noise. A collision or an illegal Windows
 * component is exactly what a user needs to see before saying yes.
 */
export function RenamePreview({ plan }: { plan: RenamePlan }) {
  const willRename = plan.items.filter((i) => !i.skipped);
  const skipped = plan.items.filter((i) => i.skipped);

  if (plan.items.length === 0) {
    return <SettingNote>Nothing to rename. Select tracks in the library.</SettingNote>;
  }

  return (
    <div className="flex flex-col gap-2">
      <p className="mono text-[12px] text-keystone-white">
        {willRename.length} renamed, {skipped.length} left alone
      </p>

      <ul className="flex flex-col gap-1">
        {willRename.slice(0, SHOWN).map((i) => (
          <li key={i.trackId} className="mono flex items-center gap-2 text-[12px]">
            <span className="min-w-0 flex-1 truncate text-gray-500" title={i.fromPath}>
              {baseName(i.fromPath)}
            </span>
            <ArrowRight2 size={11} color="currentColor" variant="Linear" className="text-gray-600" />
            <span className="min-w-0 flex-1 truncate text-keystone-lime" title={i.toPath}>
              {baseName(i.toPath)}
            </span>
          </li>
        ))}
      </ul>

      {willRename.length > SHOWN && (
        <SettingNote>and {willRename.length - SHOWN} more</SettingNote>
      )}

      {skipped.slice(0, 3).map((i) => (
        <p key={i.trackId} className="mono truncate text-[12px] text-amber-400">
          {baseName(i.fromPath)}: {i.skipped}
        </p>
      ))}
    </div>
  );
}

/** Works for both separators: a Windows plan is reviewed on Windows. */
function baseName(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i === -1 ? path : path.slice(i + 1);
}

import { Tag } from '../ui/icons';
import { useUiStore, type ActiveView } from '../../stores/uiStore';

const TABS: Array<{ id: ActiveView; label: string }> = [
  { id: 'library', label: 'Library' },
  { id: 'tags', label: 'Tags' },
];

/**
 * Library / Tags, at the top of the main column rather than in the title bar.
 *
 * They switch what the second column shows, so they belong to that column.
 * In the window title bar they sat beside the traffic lights, in the strip
 * that spans the sidebar too — which implied they governed the whole window,
 * and left the main column with no header of its own while the sidebar had a
 * title for every section in it.
 *
 * Drawn as one segmented control rather than two loose pills. Two rounded
 * buttons with a gap between them read as two independent toggles, either of
 * which might be on; a single bordered track with one solid segment reads as
 * what it is — two states of one thing, exactly one of them current.
 */
export function ViewTabs() {
  const view = useUiStore((s) => s.view);
  const setView = useUiStore((s) => s.setView);

  return (
    <nav className="flex h-10 shrink-0 items-center border-b border-gray-700 bg-gray-900 px-3">
      <div
        role="tablist"
        className="inline-flex items-center gap-0.5 rounded-md border border-gray-700 bg-gray-800 p-0.5"
      >
        {TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={view === tab.id}
            onClick={() => setView(tab.id)}
            className={`flex h-7 items-center gap-1.5 rounded px-4 text-xs font-semibold transition-colors ${
              view === tab.id
                ? 'bg-keystone-lime text-[color:var(--color-on-accent)]'
                : 'text-gray-400 hover:bg-gray-700 hover:text-keystone-white'
            }`}
          >
            {tab.id === 'tags' && (
              <Tag size={13} color="currentColor" variant="Linear" />
            )}
            {tab.label}
          </button>
        ))}
      </div>
    </nav>
  );
}

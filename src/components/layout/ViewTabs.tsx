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
 * Underlined text, with nothing boxed. Pills read as two independent
 * toggles, and a filled segmented control put a block of accent at the top
 * of the column competing with the waveform directly beneath it. The
 * underline sits ON the border this bar shares with the content below, so
 * the mark and the edge are the same line — which is what makes it read as
 * "this tab owns what is under it" rather than as another button.
 *
 * The accent is spent on the 2px rule alone. Everything else is weight and
 * colour of text.
 */
export function ViewTabs() {
  const view = useUiStore((s) => s.view);
  const setView = useUiStore((s) => s.setView);

  return (
    <nav
      role="tablist"
      className="flex h-10 shrink-0 items-end gap-5 border-b border-gray-700 bg-gray-900 px-3"
    >
      {TABS.map((tab) => {
        const active = view === tab.id;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => setView(tab.id)}
            // -mb-px pulls the underline down onto the nav's own bottom
            // border so the two occupy the same pixel row, rather than the
            // rule floating a pixel above the edge.
            className={`-mb-px flex h-9 items-center gap-1.5 border-b-2 px-0.5 text-xs font-semibold transition-colors ${
              active
                ? 'border-keystone-lime text-keystone-white'
                : 'border-transparent text-gray-400 hover:text-keystone-white'
            }`}
          >
            {tab.id === 'tags' && (
              <Tag size={13} color="currentColor" variant="Linear" />
            )}
            {tab.label}
          </button>
        );
      })}
    </nav>
  );
}

import { Input } from '../ui/Input';
import { SearchNormal } from '../ui/icons';
import { useLibraryStore } from '../../stores/libraryStore';
import { formatCount } from '../../utils/formatters';

export type TagsTab = 'replace' | 'spaces';

const TABS: Array<{ id: TagsTab; label: string }> = [
  { id: 'replace', label: 'Global Replace' },
  { id: 'spaces', label: 'Remove Extra Spaces' },
];

/**
 * The two tabs, plus the search and count.
 *
 * Search writes `filters.query` in the same store the library view reads, on
 * purpose: this is one library seen twice, and a tag cleanup almost always
 * starts from a search the user already ran next door.
 */
export function TagsToolbar({
  tab,
  onTab,
  total,
}: {
  tab: TagsTab;
  onTab: (tab: TagsTab) => void;
  total: number;
}) {
  const query = useLibraryStore((s) => s.filters.query);
  const setQuery = useLibraryStore((s) => s.setQuery);

  return (
    <div className="flex items-center gap-3 border-b border-gray-700 px-3 py-2">
      <div role="tablist" className="flex gap-1">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            onClick={() => onTab(t.id)}
            className={`h-8 rounded-md px-3 text-xs font-medium transition-colors ${
              tab === t.id
                ? 'bg-keystone-lime/15 text-keystone-lime'
                : 'text-gray-400 hover:bg-gray-800 hover:text-keystone-white'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      <Input
        className="ml-auto w-56"
        value={query}
        placeholder="Search"
        onChange={(e) => setQuery(e.target.value)}
        leading={<SearchNormal size={13} color="currentColor" variant="Linear" />}
      />
      <span className="label tnum whitespace-nowrap">{formatCount(total)} tracks</span>
    </div>
  );
}

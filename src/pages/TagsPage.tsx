import { useState } from 'react';
import { GlobalReplacePanel } from '../components/tags/GlobalReplacePanel';
import { RemoveSpacesPanel } from '../components/tags/RemoveSpacesPanel';
import { TagTable } from '../components/tags/TagTable';
import { TagsToolbar, type TagsTab } from '../components/tags/TagsToolbar';
import { TrackTagEditor } from '../components/tags/TrackTagEditor';
import { useTracks } from '../hooks/useLibrary';

/**
 * SPEC.md §2 — tag editing over the same library.
 *
 * The same `libraryStore` filters and the same `useTracks()` rows as the
 * library view: this is a second view of one library, not a second library.
 * Which track is open in the editor is local state — it is a property of this
 * view only, and putting it in a store would leave a panel open behind the
 * user when they come back to it.
 */
export function TagsPage() {
  const [tab, setTab] = useState<TagsTab>('replace');
  const [editing, setEditing] = useState<string | null>(null);
  const { data } = useTracks(0);

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col">
        <TagsToolbar tab={tab} onTab={setTab} total={data?.total ?? 0} />
        {tab === 'replace' ? <GlobalReplacePanel /> : <RemoveSpacesPanel />}
        <TagTable onOpen={setEditing} />
      </div>
      {editing && <TrackTagEditor trackId={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}

import { useState } from 'react';
import { Input } from '../ui/Input';
import { useCollectionStore } from '../../stores/collectionStore';

/**
 * The field the `+` opens.
 *
 * An empty name on blur is a cancel rather than a collection called "": a row
 * with no name cannot be clicked, renamed or told apart from the next one.
 */
export function NewCollectionField({ onCreate }: { onCreate: (name: string) => void }) {
  const setCreating = useCollectionStore((s) => s.setCreating);
  const [draft, setDraft] = useState('');

  const submit = () => {
    const name = draft.trim();
    setDraft('');
    if (name) onCreate(name);
    else setCreating(false);
  };

  return (
    <Input
      autoFocus
      value={draft}
      placeholder="Name"
      className="mt-2 h-8"
      onChange={(e) => setDraft(e.target.value)}
      onBlur={submit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') submit();
        if (e.key === 'Escape') {
          setDraft('');
          setCreating(false);
        }
      }}
    />
  );
}

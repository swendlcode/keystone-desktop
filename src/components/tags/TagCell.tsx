import { useEffect, useRef, useState } from 'react';

interface TagCellProps {
  value: string | null;
  onCommit: (value: string) => void;
  /** DM Mono, for a year or a number. */
  numeric?: boolean;
  align?: 'left' | 'right';
  className?: string;
}

/**
 * A table cell you can type into.
 *
 * Commits on blur and on Enter; Escape puts the original back. It only fires
 * `onCommit` when the text actually changed, which matters more here than it
 * looks: each commit is a write to the user's file through the safe write path
 * — copy, modify, verify, rename — and tabbing across a row of eight cells
 * should not rewrite the file eight times.
 */
export function TagCell({
  value,
  onCommit,
  numeric = false,
  align = 'left',
  className = '',
}: TagCellProps) {
  const original = value ?? '';
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(original);
  const ref = useRef<HTMLInputElement>(null);

  // A landed write, or another tagger's edit arriving by invalidation, has to
  // reach the cell — but not while it is being typed into.
  useEffect(() => {
    if (!editing) setDraft(original);
  }, [editing, original]);

  const commit = () => {
    setEditing(false);
    if (draft !== original) onCommit(draft);
  };

  if (!editing) {
    return (
      <button
        type="button"
        onClick={() => setEditing(true)}
        title={original}
        className={`block w-full truncate rounded px-1.5 py-1 text-left text-xs text-gray-300 hover:bg-gray-800 hover:text-keystone-white ${
          numeric ? 'mono tnum' : ''
        } ${align === 'right' ? 'text-right' : ''} ${className}`}
      >
        {original || <span className="text-gray-600">—</span>}
      </button>
    );
  }

  return (
    <input
      ref={ref}
      autoFocus
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') commit();
        if (e.key === 'Escape') {
          setDraft(original);
          setEditing(false);
        }
      }}
      className={`w-full rounded border border-keystone-lime bg-gray-800 px-1.5 py-1 text-xs text-keystone-white outline-none ${
        numeric ? 'mono tnum' : ''
      } ${align === 'right' ? 'text-right' : ''} ${className}`}
    />
  );
}

import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Button } from '../ui/Button';
import { Input } from '../ui/Input';
import { Music } from '../ui/icons';

/**
 * Naming a collection — new or renamed — in a dialog.
 *
 * It replaced an inline field that opened in the row itself. Two reasons.
 * The row is 28px of a 260px column, so a long crate name was edited through
 * a slot too narrow to read it in. And the gesture that opened it was a
 * double-click on the row, which is also the gesture for "I meant to click
 * and the pointer moved" — renaming is not something a user should discover
 * by accident on a collection they were only trying to select.
 *
 * The text starts selected, because the common case for a rename is
 * replacing the name rather than editing one character of it.
 */
export function CollectionNameDialog({
  title,
  initial = '',
  confirmLabel,
  busy = false,
  onConfirm,
  onCancel,
}: {
  title: string;
  initial?: string;
  confirmLabel: string;
  busy?: boolean;
  onConfirm: (name: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(initial);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.select();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !busy) onCancel();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [busy, onCancel]);

  const name = value.trim();
  const submit = () => {
    // An empty name leaves a row nothing can be read off, so it is refused
    // rather than accepted and shown as a blank.
    if (!name || busy) return;
    onConfirm(name);
  };

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      onMouseDown={(e) => e.target === e.currentTarget && !busy && onCancel()}
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 p-4"
    >
      <div className="w-full max-w-sm rounded-xl border border-gray-700 bg-gray-900 px-6 py-6 shadow-2xl">
        <Music
          size={26}
          color="currentColor"
          variant="Bold"
          className="mx-auto text-keystone-lime"
        />
        <h3 className="mt-3 text-center text-sm font-semibold text-keystone-white">
          {title}
        </h3>

        <label className="label mt-5 block" htmlFor="collection-name">
          Collection name
        </label>
        <Input
          ref={inputRef}
          id="collection-name"
          autoFocus
          value={value}
          className="mt-1.5"
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              submit();
            }
          }}
        />

        <div className="mt-6 flex items-center justify-end gap-2">
          <Button variant="ghost" size="sm" disabled={busy} onClick={onCancel}>
            Cancel
          </Button>
          <Button
            variant="primary"
            size="sm"
            disabled={busy || !name}
            onClick={submit}
          >
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

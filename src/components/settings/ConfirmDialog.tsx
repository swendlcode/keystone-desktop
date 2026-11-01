import { useEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Button } from '../ui/Button';

interface ConfirmDialogProps {
  title: string;
  /** The consequence, in a line or two. Not a lecture. */
  body?: ReactNode;
  confirmLabel: string;
  cancelLabel?: string;
  destructive?: boolean;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * The gate in front of anything that touches the user's files.
 *
 * Every write in Keystone is preceded by a dry run, and this is where that dry
 * run's counts are shown. It is deliberately not dismissible by clicking the
 * backdrop: a bulk re-tag across 5,000 files is not something to start with a
 * stray click, and the one thing worse than an extra Escape press is an
 * accidental rewrite of a library.
 *
 * Enter confirms and Escape cancels. The confirm button takes focus when the
 * dialog opens, so Enter is the native activation of a focused control with a
 * visible ring on it rather than an invisible global shortcut — which matters
 * for a dialog whose default action is usually destructive. The key handler
 * is only a backstop for when focus has moved elsewhere in the dialog, and it
 * stands down when the button already has it so a single press cannot fire
 * twice.
 *
 * Belongs in `components/ui/` — it is imported from tags, settings and the
 * context menu — but that directory is owned elsewhere, so it lives here with
 * the panes that need it most.
 */
export function ConfirmDialog({
  title,
  body,
  confirmLabel,
  cancelLabel = 'Cancel',
  destructive = false,
  busy = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (busy) return;
      if (e.key === 'Escape') {
        onCancel();
        return;
      }
      if (e.key === 'Enter') {
        // The focused button handles its own Enter; stepping in as well
        // would run the action twice.
        if (document.activeElement === confirmRef.current) return;
        e.preventDefault();
        onConfirm();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [busy, onCancel, onConfirm]);

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 p-4"
    >
      <div className="w-full max-w-md rounded-xl border border-gray-700 bg-gray-900 p-5 shadow-2xl">
        <h3 className="text-base font-semibold text-keystone-white">{title}</h3>
        {body && <div className="mt-2 text-xs text-gray-300">{body}</div>}
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" size="sm" disabled={busy} onClick={onCancel}>
            {cancelLabel}
          </Button>
          <Button
            ref={confirmRef}
            autoFocus
            variant={destructive ? 'danger' : 'primary'}
            size="sm"
            disabled={busy}
            onClick={onConfirm}
            className="focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-keystone-lime"
          >
            {busy ? 'Working...' : confirmLabel}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

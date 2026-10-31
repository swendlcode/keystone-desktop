import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { ArrowDown2 } from './icons';

interface DropdownProps {
  label: ReactNode;
  /** Highlights the trigger when a filter inside it is set. */
  active?: boolean;
  children: ReactNode;
  minWidth?: number;
  align?: 'left' | 'right';
}

/**
 * Filter / options panel. Portalled so it floats over the table and the
 * player, neither of which it can be a child of.
 *
 * Repositions on scroll and resize because the trigger lives in a toolbar that
 * moves with the window.
 */
export function Dropdown({
  label,
  active = false,
  children,
  minWidth = 240,
  align = 'left',
}: DropdownProps) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ top: 0, left: 0 });

  const reposition = useCallback(() => {
    const btn = triggerRef.current;
    if (!btn) return;
    const r = btn.getBoundingClientRect();
    let left = align === 'right' ? r.right - minWidth : r.left;
    left = Math.max(8, Math.min(left, window.innerWidth - minWidth - 8));
    setPos({ top: r.bottom + 6, left });
  }, [align, minWidth]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (
        panelRef.current &&
        !panelRef.current.contains(e.target as Node) &&
        triggerRef.current &&
        !triggerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    window.addEventListener('scroll', reposition, true);
    window.addEventListener('resize', reposition);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
      window.removeEventListener('scroll', reposition, true);
      window.removeEventListener('resize', reposition);
    };
  }, [open, reposition]);

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={() => {
          if (!open) reposition();
          setOpen((o) => !o);
        }}
        aria-expanded={open}
        aria-haspopup="true"
        className={`flex h-8 items-center gap-1.5 whitespace-nowrap rounded-md border px-3 text-xs font-medium transition-colors ${
          active || open
            ? 'border-keystone-lime bg-keystone-lime/10 text-keystone-lime'
            : 'border-gray-600 bg-transparent text-gray-300 hover:border-gray-500 hover:bg-gray-800 hover:text-keystone-white'
        }`}
      >
        {label}
        <ArrowDown2
          size={11}
          color="currentColor"
          variant="Linear"
          style={{
            transition: 'transform 150ms',
            transform: open ? 'rotate(180deg)' : 'rotate(0deg)',
          }}
        />
      </button>

      {open &&
        createPortal(
          <div
            ref={panelRef}
            style={{ ...pos, minWidth, maxWidth: 'calc(100vw - 16px)' }}
            className="fixed z-50 rounded-xl border border-gray-700 bg-gray-900 shadow-2xl shadow-black/40"
          >
            {children}
          </div>,
          document.body,
        )}
    </>
  );
}

export function DropdownSection({
  title,
  children,
}: {
  title?: string;
  children: ReactNode;
}) {
  return (
    <div className="px-3 py-3">
      {title && <div className="label mb-2">{title}</div>}
      {children}
    </div>
  );
}

export function DropdownDivider() {
  return <div className="h-px bg-gray-700/80" />;
}

import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ArrowRight2 } from '../ui/icons';
import type { MenuItem } from '../ui/menuItem';

interface MenuProps {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}

const WIDTH = 230;
const ITEM_H = 28;

/**
 * A context menu with working submenus.
 *
 * The only context-menu renderer in the app. It nests, which SPEC.md §4 needs
 * for Columns and Key System; the primitive this replaced drew a submenu's
 * chevron and never opened it, so those two rows closed the menu instead.
 *
 * Submenus are rendered *inside* the parent panel's DOM subtree while being
 * positioned `fixed`. Visually they float; structurally they are children,
 * which is what makes one outside-click listener on the root enough. A
 * separately portalled submenu would count as outside and close the menu the
 * moment the pointer reached it.
 */
export function Menu({ x, y, items, onClose }: MenuProps) {
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  return createPortal(
    <div ref={rootRef}>
      <MenuList items={items} x={x} y={y} onClose={onClose} />
    </div>,
    document.body,
  );
}

function MenuList({
  items,
  x,
  y,
  onClose,
}: {
  items: MenuItem[];
  x: number;
  y: number;
  onClose: () => void;
}) {
  const [openId, setOpenId] = useState<string | null>(null);
  const [childAt, setChildAt] = useState({ x: 0, y: 0 });

  // A right-click near the bottom of a 20,000-row table is the normal case, so
  // the panel is pulled back inside the viewport rather than drawn off it.
  const height = items.length * ITEM_H + 12;
  const left = Math.min(x, window.innerWidth - WIDTH - 8);
  const top = Math.min(y, Math.max(8, window.innerHeight - height - 8));
  const open = items.find((i) => i.id === openId);

  return (
    <div
      role="menu"
      style={{ top, left, width: WIDTH }}
      className="fixed z-50 rounded-lg border border-gray-700 bg-gray-900 py-1 shadow-2xl shadow-black/50"
    >
      {items.map((item) => (
        <div key={item.id}>
          {item.separatorBefore && <div className="my-1 h-px bg-gray-700" />}
          <Row
            item={item}
            onHover={(rect) => {
              setOpenId(item.items ? item.id : null);
              if (item.items) setChildAt({ x: rect.right - 6, y: rect.top - 5 });
            }}
            onSelect={() => {
              if (item.items) return;
              item.onSelect?.();
              onClose();
            }}
          />
        </div>
      ))}

      {open?.items && (
        <MenuList items={open.items} x={childAt.x} y={childAt.y} onClose={onClose} />
      )}
    </div>
  );
}

function Row({
  item,
  onHover,
  onSelect,
}: {
  item: MenuItem;
  onHover: (rect: DOMRect) => void;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      disabled={item.disabled}
      onMouseEnter={(e) => onHover(e.currentTarget.getBoundingClientRect())}
      onClick={onSelect}
      className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
        item.danger
          ? 'text-red-400 hover:bg-red-500/10'
          : 'text-gray-300 hover:bg-gray-800 hover:text-keystone-white'
      }`}
    >
      <span className="flex w-3.5 shrink-0 justify-center">{item.icon}</span>
      <span className="flex-1 truncate">{item.label}</span>
      {item.items && (
        <ArrowRight2 size={11} color="currentColor" variant="Linear" className="text-gray-500" />
      )}
    </button>
  );
}

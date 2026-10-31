import type { ReactNode } from 'react';

/**
 * One row of a context menu.
 *
 * A type on its own, with no component beside it, because the renderer is
 * `components/menu/Menu.tsx` and nothing else should grow a second one. There
 * used to be a `ui/ContextMenu` that drew a submenu's chevron without ever
 * opening it, which left SPEC.md §4's Columns and Key System as dead rows and
 * had every call site working around them.
 */
export interface MenuItem {
  id: string;
  label: string;
  onSelect?: () => void;
  icon?: ReactNode;
  disabled?: boolean;
  /** Draws a divider above this item. */
  separatorBefore?: boolean;
  /** A submenu — SPEC.md §4 has Columns and Key System as submenus. */
  items?: MenuItem[];
  danger?: boolean;
}

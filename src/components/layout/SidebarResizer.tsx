import { useRef, type PointerEvent as ReactPointerEvent } from 'react';
import { MIN_WIDTH, useUiStore } from '../../stores/uiStore';

/**
 * Drag below this and the column closes instead of clamping to MIN_WIDTH.
 *
 * Far enough below the minimum that a user trimming the wheel down to its
 * smallest useful size cannot shut the sidebar by overshooting, close enough
 * that closing it does not feel like a separate gesture.
 */
const COLLAPSE_BELOW = MIN_WIDTH - 56;

/**
 * The sidebar's edge: one handle that resizes it, closes it and opens it.
 *
 * This replaced a hamburger button in the title bar. A toggle up there is a
 * second place to learn, in a corner nothing else in this window uses, for a
 * column whose own edge is already under the hand that is about to resize it —
 * and it could only toggle, so the width was unreachable without a second
 * affordance somewhere else.
 *
 * The handle is rendered whether or not the sidebar is open, which is the part
 * that makes it a replacement rather than a regression: closed, it sits at the
 * window's left edge and dragging right brings the column back. Without that
 * there would be no way to reopen it at all.
 *
 * Pointer capture is what lets the drag continue over the track table — and
 * over the gap where the sidebar used to be, once a drag has closed it.
 */
export function SidebarResizer() {
  const open = useUiStore((s) => s.sidebarOpen);
  const dragging = useRef(false);

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    dragging.current = true;
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!dragging.current) return;
    const ui = useUiStore.getState();
    // The sidebar starts at the window's left edge, so the pointer's x IS the
    // width being asked for.
    const wanted = e.clientX;

    if (wanted < COLLAPSE_BELOW) {
      if (ui.sidebarOpen) ui.toggleSidebar();
      return;
    }
    if (!ui.sidebarOpen) ui.toggleSidebar();
    // setSidebarWidth clamps, so dragging past either limit parks the edge
    // there rather than letting the column run off.
    ui.setSidebarWidth(wanted);
  };

  const endDrag = (e: ReactPointerEvent<HTMLDivElement>) => {
    dragging.current = false;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={open ? 'Resize or hide the sidebar' : 'Show the sidebar'}
      title={open ? 'Drag to resize · double-click to hide' : 'Drag or double-click to show'}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onDoubleClick={() => useUiStore.getState().toggleSidebar()}
      // Negative margins pull the 7px strip over the sidebar's own 1px border
      // so the grab area straddles the visible edge rather than sitting beside
      // it, and the layout width stays what the sidebar says it is.
      className="group relative z-20 w-[7px] shrink-0 cursor-col-resize touch-none select-none -ml-[3px] -mr-[3px]"
    >
      <span className="pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-transparent transition-colors group-hover:bg-keystone-lime/60" />
      {/* Closed, the handle has no column beside it to be the edge of, so it
          shows a grip of its own — otherwise it is an invisible strip the user
          has no reason to believe is there. */}
      {!open && (
        <span className="pointer-events-none absolute left-1/2 top-1/2 h-10 w-[3px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-gray-700 transition-colors group-hover:bg-keystone-lime/70" />
      )}
    </div>
  );
}

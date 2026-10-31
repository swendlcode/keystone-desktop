import { useCallback, useState } from 'react';
import { ConfirmDialog } from '../settings/ConfirmDialog';
import { Menu } from './Menu';
import { useTrackMenu, type ConfirmRequest } from './useTrackMenu';
import { useTrack } from '../../hooks/useLibrary';
import { useUiStore } from '../../stores/uiStore';

/**
 * The library's right-click menu. Mounted once, from App.
 *
 * `uiStore.contextMenu` is the single anchor, so there is one menu at a time
 * no matter how many tables are mounted. The confirmation outlives the menu on
 * purpose: selecting an item closes the menu, and a dialog owned by the menu
 * would be unmounted in the same tick.
 */
export function TrackContextMenu() {
  const anchor = useUiStore((s) => s.contextMenu);
  const close = useUiStore((s) => s.closeContextMenu);
  const { data: track } = useTrack(anchor?.trackId ?? null);
  const [request, setRequest] = useState<ConfirmRequest | null>(null);

  const ask = useCallback((r: ConfirmRequest) => setRequest(r), []);
  const items = useTrackMenu(track ?? null, ask);

  return (
    <>
      {anchor && items.length > 0 && (
        <Menu x={anchor.x} y={anchor.y} items={items} onClose={close} />
      )}
      {request && (
        <ConfirmDialog
          title={request.title}
          body={request.body}
          confirmLabel={request.confirmLabel}
          destructive
          onCancel={() => setRequest(null)}
          onConfirm={() => {
            request.run();
            setRequest(null);
          }}
        />
      )}
    </>
  );
}

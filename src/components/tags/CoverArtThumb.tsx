import { MusicSquare } from '../ui/icons';
import { coverArtUrl } from '../../services/keystoneService';
import { useCoverArt } from './useTagEditing';

/**
 * The COVER ART cell.
 *
 * Only mounted for rows the virtualizer has on screen, which is what keeps
 * this honest: the art is a base64 frame out of the file's tag, tens of KB
 * each, and fetching it for 20,000 rows would be hundreds of megabytes over
 * IPC for pictures nobody is looking at.
 */
export function CoverArtThumb({ trackId, size = 28 }: { trackId: string; size?: number }) {
  const { data } = useCoverArt(trackId);

  if (!data) {
    return (
      <span
        style={{ width: size, height: size }}
        className="flex items-center justify-center rounded bg-gray-800 text-gray-600"
      >
        <MusicSquare size={size * 0.5} color="currentColor" variant="Linear" />
      </span>
    );
  }

  return (
    <img
      src={coverArtUrl(data)}
      alt=""
      width={size}
      height={size}
      className="rounded object-cover"
      style={{ width: size, height: size }}
    />
  );
}

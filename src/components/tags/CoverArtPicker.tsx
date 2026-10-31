import { useRef, useState } from 'react';
import { Magicpen, MusicSquare } from '../ui/icons';
import { coverArtUrl } from '../../services/keystoneService';
import { useCoverArt, useSetCoverArt } from './useTagEditing';

/**
 * The artwork, click to replace.
 *
 * It reads the picked image with a `FileReader` rather than through the fs
 * plugin. The result is a data URL, which is already `mime` plus base64 — the
 * exact two fields `CoverArt` carries — and it needs no filesystem permission
 * for a path the user chose in a dialog this frame.
 */
export function CoverArtPicker({ trackId }: { trackId: string }) {
  const { data } = useCoverArt(trackId);
  const setArt = useSetCoverArt();
  const inputRef = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);

  const onPick = (file: File) => {
    setError(null);
    const reader = new FileReader();
    reader.onload = () => {
      const url = String(reader.result);
      const match = /^data:([^;]+);base64,(.*)$/.exec(url);
      if (!match) {
        setError('That image could not be read.');
        return;
      }
      setArt.mutate({ trackId, art: { mime: match[1], dataBase64: match[2] } });
    };
    reader.onerror = () => setError('That image could not be read.');
    reader.readAsDataURL(file);
  };

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        onClick={() => inputRef.current?.click()}
        className="group/art relative h-32 w-32 overflow-hidden rounded-lg border border-gray-700 bg-gray-800"
      >
        {data ? (
          <img src={coverArtUrl(data)} alt="" className="h-full w-full object-cover" />
        ) : (
          <span className="flex h-full w-full items-center justify-center text-gray-600">
            <MusicSquare size={28} color="currentColor" variant="Linear" />
          </span>
        )}
        <span className="absolute inset-0 flex items-center justify-center gap-1.5 bg-black/70 text-[11px] text-keystone-white opacity-0 transition-opacity group-hover/art:opacity-100">
          <Magicpen size={12} color="currentColor" variant="Linear" />
          Replace
        </span>
      </button>

      {error && <p className="text-[11px] text-amber-400">{error}</p>}
      {setArt.isPending && <p className="text-[11px] text-gray-500">Writing...</p>}

      <input
        ref={inputRef}
        type="file"
        accept="image/jpeg,image/png"
        className="hidden"
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) onPick(file);
          // Cleared so picking the same file twice fires change again.
          e.target.value = '';
        }}
      />
    </div>
  );
}

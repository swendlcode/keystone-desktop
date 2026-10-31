import { keyName } from '../../utils/camelot';
import type { KeyMode, KeyNotation } from '../../types';

/**
 * Scale degrees as semitones above the tonic.
 *
 * Natural minor, not harmonic or melodic: the engine's key model is the
 * 24-key major/minor taxonomy, so a raised seventh here would be drawing a
 * scale it never claimed to have found.
 */
const SCALE: Record<KeyMode, number[]> = {
  major: [0, 2, 4, 5, 7, 9, 11],
  minor: [0, 2, 3, 5, 7, 8, 10],
};

/** White pitch classes in order, and the black ones with the white they follow. */
const WHITE = [0, 2, 4, 5, 7, 9, 11];
const BLACK: Array<{ pc: number; afterWhite: number }> = [
  { pc: 1, afterWhite: 0 },
  { pc: 3, afterWhite: 1 },
  { pc: 6, afterWhite: 3 },
  { pc: 8, afterWhite: 4 },
  { pc: 10, afterWhite: 5 },
];

const OCTAVES = 2;

interface VirtualPianoProps {
  keyRoot: number | null;
  keyMode: KeyMode | null;
  notation: KeyNotation;
}

/**
 * The keyboard behind VIRTUAL PIANO, showing the detected key's scale.
 *
 * Two octaves, because one does not show a DJ the shape of the scale and
 * three does not fit the width the player has. The tonic carries the accent
 * and the other six degrees a tint of it: the thing being answered is "what
 * notes can I play over this", and the tonic is the one that has to be
 * findable at a glance.
 */
export function VirtualPiano({ keyRoot, keyMode, notation }: VirtualPianoProps) {
  if (keyRoot == null || keyMode == null) {
    return (
      <p className="label px-1 py-2">No key detected</p>
    );
  }

  const degrees = new Set(SCALE[keyMode].map((d) => (keyRoot + d) % 12));
  const whiteCount = WHITE.length * OCTAVES;
  const whiteWidth = 100 / whiteCount;
  const name = keyName(keyRoot, keyMode, notation === 'flats' ? 'flats' : 'sharps');

  return (
    <div className="flex items-center gap-3 px-1 pb-1 pt-2">
      <span className="mono shrink-0 text-xs text-keystone-lime">{name}</span>
      <div className="relative h-14 min-w-0 flex-1 select-none">
        <div className="flex h-full w-full gap-px">
          {Array.from({ length: whiteCount }, (_, i) => {
            const pc = WHITE[i % WHITE.length];
            return (
              <Key
                key={`w${i}`}
                pitchName={keyName(pc, 'major', notation === 'flats' ? 'flats' : 'sharps')}
                tonic={pc === keyRoot % 12}
                inScale={degrees.has(pc)}
                white
              />
            );
          })}
        </div>

        <div className="pointer-events-none absolute inset-0">
          {Array.from({ length: OCTAVES }, (_, oct) =>
            BLACK.map(({ pc, afterWhite }) => {
              const whiteIndex = oct * WHITE.length + afterWhite;
              return (
                <span
                  key={`b${oct}-${pc}`}
                  style={{
                    left: `${(whiteIndex + 1) * whiteWidth}%`,
                    width: `${whiteWidth * 0.6}%`,
                  }}
                  className="absolute top-0 h-[62%] -translate-x-1/2"
                >
                  <Key
                    pitchName=""
                    tonic={pc === keyRoot % 12}
                    inScale={degrees.has(pc)}
                    white={false}
                  />
                </span>
              );
            }),
          )}
        </div>
      </div>
    </div>
  );
}

function Key({
  pitchName,
  tonic,
  inScale,
  white,
}: {
  pitchName: string;
  tonic: boolean;
  inScale: boolean;
  white: boolean;
}) {
  const base = white
    ? 'bg-gray-200 text-gray-600'
    : 'bg-gray-900 text-gray-400 ring-1 ring-black/60';
  const lit = tonic
    ? 'bg-keystone-lime text-[color:var(--color-on-accent)]'
    : inScale
      ? white
        ? 'bg-keystone-lime/45 text-[color:var(--color-on-accent)]'
        : 'bg-keystone-moss/70 text-[color:var(--color-on-accent)]'
      : base;

  return (
    <span
      className={`flex h-full w-full items-end justify-center rounded-b-sm pb-1 text-[9px] ${lit}`}
    >
      {white && pitchName}
    </span>
  );
}

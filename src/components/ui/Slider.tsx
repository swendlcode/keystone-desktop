import type { ChangeEvent } from 'react';

interface SliderProps {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange: (value: number) => void;
  className?: string;
  disabled?: boolean;
  'aria-label'?: string;
}

/**
 * Range input styled by `.ks-slider` in index.css.
 *
 * The filled portion is drawn by `--slider-pct` on the track gradient rather
 * than by a second element, so there is nothing to keep in sync while dragging.
 */
export function Slider({
  value,
  min = 0,
  max = 100,
  step = 1,
  onChange,
  className = '',
  disabled = false,
  'aria-label': ariaLabel,
}: SliderProps) {
  const span = max - min;
  const pct = span > 0 ? ((value - min) / span) * 100 : 0;

  return (
    <input
      type="range"
      className={`ks-slider ${className}`}
      style={{ ['--slider-pct' as string]: `${pct}%` }}
      value={value}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      aria-label={ariaLabel}
      onChange={(e: ChangeEvent<HTMLInputElement>) =>
        onChange(Number(e.target.value))
      }
    />
  );
}

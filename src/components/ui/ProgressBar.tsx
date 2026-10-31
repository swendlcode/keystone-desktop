interface ProgressBarProps {
  /** 0-1. Null draws an indeterminate bar. */
  value: number | null;
  className?: string;
}

/**
 * Thin accent bar for the analysis queue and the export runs.
 *
 * `null` is indeterminate rather than zero, which is the honest rendering
 * while the queue has work but no measured rate — the engine's timings are all
 * from one Mac and there is no Windows measurement to extrapolate from.
 */
export function ProgressBar({ value, className = '' }: ProgressBarProps) {
  const pct = value == null ? 100 : Math.max(0, Math.min(1, value)) * 100;
  return (
    <div className={`h-1 w-full overflow-hidden rounded-full bg-gray-700 ${className}`}>
      <div
        className={`h-full rounded-full bg-keystone-lime transition-[width] duration-300 ${
          value == null ? 'opacity-40' : ''
        }`}
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}

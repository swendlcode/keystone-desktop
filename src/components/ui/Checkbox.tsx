import type { ReactNode } from 'react';

interface CheckboxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: ReactNode;
  disabled?: boolean;
  /** Shown under the label — the "why this is disabled" line. */
  hint?: ReactNode;
  className?: string;
  'aria-label'?: string;
}

/**
 * The Settings checkbox. A hidden native input drives accessibility and the
 * visual layer is ours.
 *
 * `hint` exists because SPEC.md §3 disables several of these with an
 * explanation (an export target whose app is not installed), and a disabled
 * checkbox with no reason beside it reads as a bug.
 */
export function Checkbox({
  checked,
  onChange,
  label,
  disabled = false,
  hint,
  className = '',
  'aria-label': ariaLabel,
}: CheckboxProps) {
  return (
    <label
      className={`group/cb flex select-none items-start gap-2 ${
        disabled ? 'cursor-not-allowed opacity-40' : 'cursor-pointer'
      } ${className}`}
    >
      <span className="relative mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center">
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          aria-label={ariaLabel}
          onChange={(e) => onChange(e.target.checked)}
          className="peer absolute inset-0 cursor-pointer opacity-0 disabled:cursor-not-allowed"
        />
        <span
          className={`flex h-4 w-4 items-center justify-center rounded border transition-colors ${
            checked
              ? 'border-keystone-lime bg-keystone-lime'
              : 'border-gray-600 bg-transparent peer-hover:border-gray-400'
          }`}
        >
          {checked && (
            <svg width="10" height="8" viewBox="0 0 10 8" fill="none" aria-hidden="true">
              <path
                d="M1 3.5L3.8 6.5L9 1"
                stroke="var(--color-on-accent)"
                strokeWidth="1.6"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          )}
        </span>
      </span>

      {(label || hint) && (
        <span className="min-w-0">
          {label && (
            <span className="block text-xs text-gray-300 group-hover/cb:text-keystone-white">
              {label}
            </span>
          )}
          {hint && <span className="mt-0.5 block text-[12px] text-gray-500">{hint}</span>}
        </span>
      )}
    </label>
  );
}

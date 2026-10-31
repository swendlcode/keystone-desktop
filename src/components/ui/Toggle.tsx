interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  disabled?: boolean;
  className?: string;
  'aria-label'?: string;
}

/** On/off switch, for the Settings rows that read as a state rather than a
    choice (`Update common tags`, `Overwrite cue points already in the
    collection`). */
export function Toggle({
  checked,
  onChange,
  label,
  disabled = false,
  className = '',
  'aria-label': ariaLabel,
}: ToggleProps) {
  return (
    <label
      className={`flex select-none items-center gap-3 ${
        disabled ? 'cursor-not-allowed opacity-40' : 'cursor-pointer'
      } ${className}`}
    >
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={ariaLabel ?? label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${
          checked ? 'bg-keystone-lime' : 'bg-gray-600'
        }`}
      >
        <span
          className="absolute top-0.5 h-4 w-4 rounded-full bg-white transition-transform"
          style={{ transform: `translateX(${checked ? 18 : 2}px)` }}
        />
      </button>
      {label && <span className="text-xs text-gray-300">{label}</span>}
    </label>
  );
}

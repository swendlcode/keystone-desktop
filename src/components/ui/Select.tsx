import { ArrowDown2 } from './icons';

interface SelectProps<T extends string> {
  value: T;
  options: Array<{ value: T; label: string }>;
  onChange: (value: T) => void;
  disabled?: boolean;
  className?: string;
  'aria-label'?: string;
}

/** Native select in Keystone's chrome. Native because the options lists here
    are short and a custom listbox buys nothing but keyboard bugs. */
export function Select<T extends string>({
  value,
  options,
  onChange,
  disabled = false,
  className = '',
  'aria-label': ariaLabel,
}: SelectProps<T>) {
  return (
    <div
      className={`relative flex h-9 items-center rounded-md border border-gray-600 bg-gray-700 text-sm focus-within:border-keystone-lime ${className}`}
    >
      <select
        value={value}
        disabled={disabled}
        aria-label={ariaLabel}
        onChange={(e) => onChange(e.target.value as T)}
        className="h-full w-full appearance-none bg-transparent pl-3 pr-8 text-keystone-white outline-none disabled:cursor-not-allowed disabled:opacity-50"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value} className="bg-gray-800">
            {o.label}
          </option>
        ))}
      </select>
      <ArrowDown2
        size={12}
        color="currentColor"
        variant="Linear"
        className="pointer-events-none absolute right-2.5 text-gray-400"
      />
    </div>
  );
}

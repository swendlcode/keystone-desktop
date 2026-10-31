interface RadioGroupProps<T extends string> {
  value: T;
  options: Array<{ value: T; label: string; hint?: string }>;
  onChange: (value: T) => void;
  name: string;
  disabled?: boolean;
  className?: string;
}

/**
 * Exactly-one-of, which is most of SPEC.md §3: what to write, where to write
 * it, the rename pattern, the key notation, the decimal places.
 *
 * Generic over the option union so a typo in an option value is a type error
 * rather than a radio that never selects.
 */
export function RadioGroup<T extends string>({
  value,
  options,
  onChange,
  name,
  disabled = false,
  className = '',
}: RadioGroupProps<T>) {
  return (
    <div className={`flex flex-col gap-2 ${className}`} role="radiogroup">
      {options.map((opt) => (
        <label
          key={opt.value}
          className={`group/r flex items-start gap-2 ${
            disabled ? 'cursor-not-allowed opacity-40' : 'cursor-pointer'
          }`}
        >
          <span className="relative mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center">
            <input
              type="radio"
              name={name}
              value={opt.value}
              checked={value === opt.value}
              disabled={disabled}
              onChange={() => onChange(opt.value)}
              className="peer absolute inset-0 cursor-pointer opacity-0 disabled:cursor-not-allowed"
            />
            <span
              className={`flex h-4 w-4 items-center justify-center rounded-full border transition-colors ${
                value === opt.value
                  ? 'border-keystone-lime'
                  : 'border-gray-600 peer-hover:border-gray-400'
              }`}
            >
              {value === opt.value && (
                <span className="h-2 w-2 rounded-full bg-keystone-lime" />
              )}
            </span>
          </span>
          <span className="min-w-0">
            <span className="block text-xs text-gray-300 group-hover/r:text-keystone-white">
              {opt.label}
            </span>
            {opt.hint && (
              <span className="mono mt-0.5 block text-[11px] text-gray-500">
                {opt.hint}
              </span>
            )}
          </span>
        </label>
      ))}
    </div>
  );
}

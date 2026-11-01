import type { ReactNode } from 'react';

/** One pane: a title and its rows. Panes never scroll themselves — the page does. */
export function SettingSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-4">
      <h2 className="text-base font-semibold text-keystone-white">{title}</h2>
      {children}
    </section>
  );
}

/** A labelled group inside a pane — "What to write", "Where to write it". */
export function SettingGroup({
  label,
  hint,
  disabled = false,
  children,
}: {
  label?: string;
  hint?: string;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      className={`flex flex-col gap-2 rounded-lg border border-gray-700 bg-gray-800 p-4 ${
        disabled ? 'opacity-40' : ''
      }`}
    >
      {label && <span className="label">{label}</span>}
      {hint && <p className="text-[12px] text-gray-400">{hint}</p>}
      {children}
    </div>
  );
}

/** Label on the left, control on the right. The default shape of a setting. */
export function SettingRow({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <span className="min-w-0">
        <span className="block text-xs text-gray-300">{label}</span>
        {hint && <span className="mt-0.5 block text-[12px] text-gray-500">{hint}</span>}
      </span>
      {children && <span className="shrink-0">{children}</span>}
    </div>
  );
}

/**
 * An inline note. `tone` carries whether it is context or a consequence —
 * SPEC.md disables several controls with an explanation, and a disabled
 * control with no reason beside it reads as a bug.
 */
export function SettingNote({
  tone = 'info',
  children,
}: {
  tone?: 'info' | 'warn';
  children: ReactNode;
}) {
  return (
    <p
      className={`text-[12px] leading-relaxed ${
        tone === 'warn' ? 'text-amber-400' : 'text-gray-500'
      }`}
    >
      {children}
    </p>
  );
}

import type { ReactNode } from 'react';

interface EmptyStateProps {
  title: string;
  /** One line. Short, per the house copy rule. */
  hint?: string;
  icon?: ReactNode;
  action?: ReactNode;
}

export function EmptyState({ title, hint, icon, action }: EmptyStateProps) {
  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-3 px-6 text-center">
      {icon && <span className="text-gray-600">{icon}</span>}
      <h2 className="text-base font-semibold text-keystone-white">{title}</h2>
      {hint && <p className="max-w-sm text-xs text-gray-400">{hint}</p>}
      {action}
    </div>
  );
}

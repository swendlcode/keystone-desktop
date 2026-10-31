import type { ReactNode } from 'react';

interface TooltipProps {
  label: string;
  children: ReactNode;
  className?: string;
}

/**
 * Native `title` for now. Kept as a component so the call sites do not change
 * when it becomes a positioned popover.
 */
export function Tooltip({ label, children, className }: TooltipProps) {
  return (
    <span title={label} className={className}>
      {children}
    </span>
  );
}

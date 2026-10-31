import type { ReactNode } from 'react';

type Tone = 'neutral' | 'accent' | 'muted' | 'outline' | 'warn';

interface BadgeProps {
  children: ReactNode;
  tone?: Tone;
  className?: string;
  title?: string;
}

const TONES: Record<Tone, string> = {
  neutral: 'bg-gray-700 text-keystone-white',
  accent: 'bg-keystone-lime text-[color:var(--color-on-accent)]',
  muted: 'bg-gray-800 text-gray-400',
  outline: 'border border-gray-600 bg-transparent text-gray-300',
  warn: 'bg-amber-500/20 text-amber-400',
};

/** `.badge` supplies DM Mono and the 11px size — every badge holds a number. */
export function Badge({ children, tone = 'neutral', className = '', title }: BadgeProps) {
  return (
    <span title={title} className={`badge ${TONES[tone]} ${className}`}>
      {children}
    </span>
  );
}

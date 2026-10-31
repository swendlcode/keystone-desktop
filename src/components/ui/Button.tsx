import { forwardRef } from 'react';
import type { ButtonHTMLAttributes, ReactNode } from 'react';

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
type Size = 'sm' | 'md' | 'lg';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  icon?: ReactNode;
}

// `text-[color:var(--color-on-accent)]` rather than a theme token: the accent
// is a bright lime in both themes, so its foreground is near-black in both and
// must NOT invert with the theme.
const VARIANTS: Record<Variant, string> = {
  primary:
    'bg-keystone-lime text-[color:var(--color-on-accent)] hover:bg-[color:var(--color-accent-hover)]',
  secondary: 'bg-gray-700 text-keystone-white hover:bg-gray-600',
  ghost:
    'bg-transparent text-gray-300 hover:bg-gray-700 hover:text-keystone-white',
  danger: 'bg-red-500/80 text-white hover:bg-red-500',
};

const SIZES: Record<Size, string> = {
  sm: 'h-7 px-3 text-xs',
  md: 'h-9 px-4 text-sm',
  lg: 'h-11 px-5 text-base',
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { variant = 'secondary', size = 'md', icon, className = '', children, ...rest },
    ref,
  ) => (
    <button
      ref={ref}
      className={`inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-md font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${VARIANTS[variant]} ${SIZES[size]} ${className}`}
      {...rest}
    >
      {icon}
      {children}
    </button>
  ),
);
Button.displayName = 'Button';

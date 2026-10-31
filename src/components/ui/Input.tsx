import { forwardRef } from 'react';
import type { InputHTMLAttributes, ReactNode } from 'react';

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  leading?: ReactNode;
  trailing?: ReactNode;
  /** DM Mono for the field itself — for BPM, cents, a port number. */
  numeric?: boolean;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ leading, trailing, numeric = false, className = '', ...rest }, ref) => (
    <div
      className={`flex h-9 items-center gap-2 rounded-md border border-gray-600 bg-gray-700 px-3 text-sm text-keystone-white focus-within:border-keystone-lime ${className}`}
    >
      {leading}
      <input
        ref={ref}
        className={`min-w-0 flex-1 bg-transparent outline-none placeholder:text-gray-400 ${numeric ? 'mono tnum' : ''}`}
        {...rest}
      />
      {trailing}
    </div>
  ),
);
Input.displayName = 'Input';

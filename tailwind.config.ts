import type { Config } from 'tailwindcss';

// Every palette entry resolves through a CSS custom property so that classes
// like `bg-gray-900`, `text-keystone-white` and `bg-keystone-lime` flip
// automatically under `[data-theme="light"]` (see src/index.css). Names keep
// their dark-theme semantics; the light theme supplies inverted values under
// the same names.
//
// The vars hold `R G B` triplets, which is what lets Tailwind's <alpha-value>
// placeholder compose opacity modifiers: `bg-keystone-lime/10` becomes
// `rgb(var(--keystone-lime) / 0.1)`.

/** The 24 Camelot swatches, generated so `bg-camelot-8a` and friends exist. */
const camelot = Object.fromEntries(
  Array.from({ length: 12 }, (_, i) => i + 1).flatMap((n) =>
    (['a', 'b'] as const).map((letter) => [
      `${n}${letter}`,
      `rgb(var(--camelot-${n}${letter}) / <alpha-value>)`,
    ]),
  ),
);

export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        keystone: {
          black: 'rgb(var(--keystone-black) / <alpha-value>)',
          lime: 'rgb(var(--keystone-lime) / <alpha-value>)',
          moss: 'rgb(var(--keystone-moss) / <alpha-value>)',
          white: 'rgb(var(--keystone-white) / <alpha-value>)',
        },
        gray: {
          900: 'rgb(var(--gray-900) / <alpha-value>)',
          800: 'rgb(var(--gray-800) / <alpha-value>)',
          700: 'rgb(var(--gray-700) / <alpha-value>)',
          600: 'rgb(var(--gray-600) / <alpha-value>)',
          500: 'rgb(var(--gray-500) / <alpha-value>)',
          400: 'rgb(var(--gray-400) / <alpha-value>)',
          300: 'rgb(var(--gray-300) / <alpha-value>)',
          200: 'rgb(var(--gray-200) / <alpha-value>)',
          100: 'rgb(var(--gray-100) / <alpha-value>)',
        },
        camelot,
      },
      fontFamily: {
        sans: ['Syne', 'system-ui', 'sans-serif'],
        mono: ['"DM Mono"', 'ui-monospace', 'monospace'],
      },
      // One step up from Tailwind's defaults (12/14/16). Syne runs small for
      // its point size and this table is read at arm's length over a mixer, so
      // the stock scale was legible rather than comfortable. Overridden here
      // rather than at each call site so the whole app moves together; the
      // explicit `text-[Npx]` values in the settings panes were raised by the
      // same single step.
      fontSize: {
        xs: ['13px', '1.45'],
        sm: ['15px', '1.45'],
        base: ['17px', '1.5'],
      },
    },
  },
  plugins: [],
} satisfies Config;

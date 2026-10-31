interface LogoMarkProps {
  size?: number;
  className?: string;
  /** Override the two stacked-plate fills. Defaults to the brand pair. */
  color?: string;
  shade?: string;
}

/**
 * Keystone's icon: two rotated, rounded plates, the back one in the accent
 * shade and the front one in the accent.
 *
 * Converted from `keystone-assets/Keystone Icon.svg` so the colours can be set
 * per use — the title bar wants the brand pair, a disabled state wants grey,
 * and a white-on-accent context wants both plates white.
 */
export function LogoMark({
  size = 28,
  className,
  color = 'rgb(var(--keystone-lime))',
  shade = 'rgb(var(--keystone-moss))',
}: LogoMarkProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 117 117"
      fill="none"
      className={className}
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <rect
        width="79.2391"
        height="79.2391"
        rx="14.5575"
        transform="matrix(0.965916 -0.258855 -0.258855 0.965916 39.9208 39.9224)"
        fill={shade}
      />
      <rect
        width="79.2391"
        height="79.2391"
        rx="14.5575"
        transform="matrix(0.965916 -0.258855 -0.258855 0.965916 20.5115 20.5115)"
        fill={color}
      />
    </svg>
  );
}

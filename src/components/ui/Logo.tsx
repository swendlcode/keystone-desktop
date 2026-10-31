import { LogoMark } from './LogoMark';

interface LogoProps {
  /** Height of the mark in px; the wordmark scales with it. */
  size?: number;
  className?: string;
  /** Mark only, for a collapsed sidebar or a narrow title bar. */
  markOnly?: boolean;
}

/**
 * Mark plus wordmark.
 *
 * The wordmark is live text in Syne rather than the logo SVG's outlined paths:
 * it inherits the theme foreground, so it stays legible when the app flips to
 * light, which a baked fill would not.
 */
export function Logo({ size = 24, className = '', markOnly = false }: LogoProps) {
  return (
    <span className={`inline-flex items-center gap-2 ${className}`}>
      <LogoMark size={size} />
      {!markOnly && (
        <span
          className="font-semibold tracking-tight text-keystone-white"
          style={{ fontSize: size * 0.62 }}
        >
          Keystone
        </span>
      )}
    </span>
  );
}

interface RatingProps {
  /** 0-5. Null is treated as 0. */
  value: number | null;
  onChange?: (value: number) => void;
  className?: string;
}

const STARS = [1, 2, 3, 4, 5];

/**
 * The RATING column: five dots, per SPEC.md §1 — not five stars.
 *
 * Clicking the dot that is already the rating clears it. Without that, a
 * mis-click on the first dot leaves a 1 the user cannot undo without a context
 * menu, and 1 and "unrated" are different things when sorting.
 */
export function Rating({ value, onChange, className = '' }: RatingProps) {
  const current = value ?? 0;
  const readOnly = !onChange;

  return (
    <span className={`inline-flex items-center gap-1 ${className}`}>
      {STARS.map((n) => (
        <button
          key={n}
          type="button"
          disabled={readOnly}
          aria-label={`${n} of 5`}
          // A 2px target must not double as a drag handle: without this a
          // press that jiggles starts dragging the row and swallows the click
          // that was setting the rating.
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => {
            e.stopPropagation();
            onChange?.(current === n ? 0 : n);
          }}
          className={`h-2 w-2 rounded-full transition-colors ${
            n <= current ? 'bg-keystone-lime' : 'bg-gray-600'
          } ${readOnly ? '' : 'hover:bg-keystone-moss'}`}
        />
      ))}
    </span>
  );
}

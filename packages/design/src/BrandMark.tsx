/** Sinua's mark: a nucleus and six dots around it, the smallest thing the engine draws. */
export function BrandMark({ className = "brand-mark", size }: { className?: string; size?: number }) {
  return (
    <svg className={className} viewBox="0 0 32 32" width={size} height={size} aria-hidden="true" fill="currentColor">
      <circle cx="16" cy="16" r="4" />
      <circle cx="16" cy="5" r="2.2" />
      <circle cx="26" cy="11" r="2.2" />
      <circle cx="26" cy="21" r="2.2" />
      <circle cx="16" cy="27" r="2.2" />
      <circle cx="6" cy="21" r="2.2" />
      <circle cx="6" cy="11" r="2.2" />
    </svg>
  );
}

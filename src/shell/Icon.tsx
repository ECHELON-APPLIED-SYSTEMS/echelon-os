/** Small local vectors avoid platform-dependent symbol fonts and icon packages. */
export function Icon({ kind }: { kind: 'overview' | 'navigation' }) {
  return (
    <svg
      className="icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {kind === 'overview' ? (
        <>
          <rect x="4" y="4" width="16" height="16" rx="1" />
          <path d="M10 4v16M10 10h10" />
        </>
      ) : (
        <>
          <path d="m5 19 14-14M8 5h11v11" />
          <path d="M5 10v9h9" />
        </>
      )}
    </svg>
  );
}

/**
 * The way back (Phase 16): an arrow and the name of where it goes, "← Home". The back bar under the masthead uses
 * it, and so does a full-screen page that covers the masthead (Who's watching?, round 10), so every way back in
 * the app looks and reads the same.
 */
export function BackLink({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={`Back to ${label}`}
      className="group -ml-2 flex h-11 items-center gap-3 rounded-sm px-2 text-[17px] text-ink-muted transition-colors duration-[var(--dur-standard)] hover:text-ink"
    >
      <svg width="22" height="22" viewBox="0 0 22 22" fill="none" aria-hidden>
        <path d="M13.5 5L7.5 11l6 6" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      <span className="max-w-[48ch] truncate">{label}</span>
    </button>
  );
}

import React from "react";

interface State {
  error: Error | null;
}

/**
 * Global error boundary (SPEC.md Phase 1, subtask 1.10).
 *
 * A React render error must not leave a blank window. The Rust side has its own
 * panic handler that writes a crash report; this is the frontend half, and it
 * shows where the log went rather than just apologising.
 */
export class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  State
> {
  override state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  override componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error("render error", error, info.componentStack);
  }

  override render() {
    const { error } = this.state;
    if (!error) return this.props.children;

    return (
      <div className="flex h-full flex-col items-center justify-center gap-5 bg-base p-10 text-center">
        <div className="text-2xl font-semibold text-ink">Something broke in the interface</div>
        <p className="max-w-lg text-sm leading-relaxed text-ink-muted">
          The rest of the application is still running. A crash report has been written
          to <code className="text-ink">data/logs/</code> next to the executable — nothing
          was sent anywhere.
        </p>
        <pre className="max-w-2xl overflow-auto rounded-lg border border-line bg-surface p-4 text-left text-xs text-ink-muted">
          {error.message}
        </pre>
        <button
          type="button"
          onClick={() => this.setState({ error: null })}
          className="rounded-md bg-oxblood px-5 py-2.5 text-sm font-medium text-ink transition-colors hover:bg-oxblood-hover"
        >
          Try again
        </button>
      </div>
    );
  }
}

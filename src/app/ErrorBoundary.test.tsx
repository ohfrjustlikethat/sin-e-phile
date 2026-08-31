import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ErrorBoundary } from "@/app/ErrorBoundary";

function Boom(): React.ReactNode {
  throw new Error("deliberate test failure");
}

describe("ErrorBoundary", () => {
  it("shows a real message instead of a blank window", () => {
    // React logs the caught error; silence it so the test output stays readable.
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );
    expect(screen.getByText(/Something broke in the interface/i)).toBeInTheDocument();
    // It must say where the crash report went — §2.7's promise is only credible
    // if the user can see the thing that was not sent anywhere.
    expect(screen.getByText(/data\/logs\//)).toBeInTheDocument();
    spy.mockRestore();
  });

  it("renders children when nothing is wrong", () => {
    render(
      <ErrorBoundary>
        <p>all good</p>
      </ErrorBoundary>,
    );
    expect(screen.getByText("all good")).toBeInTheDocument();
  });
});

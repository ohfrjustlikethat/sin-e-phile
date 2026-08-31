import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Placeholder } from "@/app/Placeholder";
import { DESTINATIONS } from "@/lib/store";

describe("Placeholder", () => {
  it("has designed copy for every destination", () => {
    // SPEC.md §4: every empty state is designed, never blank. A destination
    // without copy would render an empty screen, which is the anti-pattern.
    for (const d of DESTINATIONS) {
      const { unmount } = render(<Placeholder destination={d} />);
      expect(screen.getByRole("heading").textContent?.length ?? 0).toBeGreaterThan(0);
      unmount();
    }
  });

  it("states which phase makes the destination real", () => {
    render(<Placeholder destination="live" />);
    // Honest about being unfinished rather than pretending otherwise.
    expect(screen.getByText(/Phase 24/i)).toBeInTheDocument();
  });

  it("does not promise content the app ships", () => {
    render(<Placeholder destination="live" />);
    expect(screen.getByText(/You supply the playlist; none ships/i)).toBeInTheDocument();
  });
});

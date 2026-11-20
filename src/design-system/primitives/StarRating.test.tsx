import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { StarRating, StarsShown, starWords } from "./StarRating";

/** ADR-0056: half stars, 0.5 to 5, rated by key, pad (a range input) or mouse. */
describe("StarRating", () => {
  const sized = (el: HTMLElement) =>
    vi.spyOn(el, "getBoundingClientRect").mockReturnValue({ left: 0, width: 100, top: 0, height: 20 } as DOMRect);

  it("says the rating in words, and Not rated before one is given", () => {
    const { rerender } = render(<StarRating value={null} onChange={() => {}} />);
    expect(screen.getByRole("slider", { name: "Your rating" })).toHaveAttribute("aria-valuetext", "Not rated");
    rerender(<StarRating value={7} onChange={() => {}} />);
    expect(screen.getByRole("slider", { name: "Your rating" })).toHaveAttribute("aria-valuetext", "3½ stars");
  });

  it("the arrow keys and the pad move it a half star at a time, and down to nothing clears it", () => {
    const onChange = vi.fn();
    render(<StarRating value={8} onChange={onChange} />);
    const slider = screen.getByRole("slider", { name: "Your rating" });
    fireEvent.change(slider, { target: { value: "9" } });
    expect(onChange).toHaveBeenLastCalledWith(9);
    fireEvent.change(slider, { target: { value: "0" } });
    expect(onChange).toHaveBeenLastCalledWith(null);
  });

  it("a click on the left half of a star is a half star; clicking the same rating clears it", () => {
    const onChange = vi.fn();
    const { container, rerender } = render(<StarRating value={null} onChange={onChange} />);
    const box = container.firstElementChild as HTMLElement;
    sized(box);
    fireEvent.click(box, { clientX: 65 });
    expect(onChange).toHaveBeenLastCalledWith(7);
    fireEvent.click(box, { clientX: 100 });
    expect(onChange).toHaveBeenLastCalledWith(10);
    rerender(<StarRating value={7} onChange={onChange} />);
    fireEvent.click(box, { clientX: 61 });
    expect(onChange).toHaveBeenLastCalledWith(null);
  });

  it("hovering previews without rating", () => {
    const onChange = vi.fn();
    const { container } = render(<StarRating value={2} onChange={onChange} />);
    const box = container.firstElementChild as HTMLElement;
    sized(box);
    fireEvent.mouseMove(box, { clientX: 95 });
    expect(onChange).not.toHaveBeenCalled();
  });

  it("an average shows as stars that only show", () => {
    render(<StarsShown halves={8.5} label="Your average: 4¼ stars" />);
    expect(screen.getByRole("img", { name: "Your average: 4¼ stars" })).toBeInTheDocument();
    expect(screen.queryByRole("slider")).toBeNull();
  });

  it("words", () => {
    expect(starWords(1)).toBe("½ star");
    expect(starWords(2)).toBe("1 star");
    expect(starWords(9)).toBe("4½ stars");
    expect(starWords(10)).toBe("5 stars");
  });
});

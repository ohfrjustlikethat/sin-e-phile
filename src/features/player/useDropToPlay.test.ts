import { describe, expect, it } from "vitest";
import { isVideoPath, titleFromPath } from "./useDropToPlay";

describe("drop to play", () => {
  it("accepts videos and refuses everything else", () => {
    expect(isVideoPath("C:\\Films\\Sunrise.1927.mkv")).toBe(true);
    expect(isVideoPath("C:\\Films\\cover.jpg")).toBe(false);
  });
  it("makes a readable title from a file name", () => {
    expect(titleFromPath("C:\\Films\\Sunrise.1927.mkv")).toBe("Sunrise 1927");
    expect(titleFromPath("/home/x/the_general.mp4")).toBe("the general");
  });
});

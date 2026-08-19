import { describe, expect, it } from "vitest";

import { formatEta } from "./eta";

describe("formatEta", () => {
  it("counts seconds under a minute", () => {
    expect(formatEta(15_000)).toBe("~15s left");
    expect(formatEta(1_000)).toBe("~1s left");
  });

  it("switches to minutes, and drops a zero seconds", () => {
    expect(formatEta(90_000)).toBe("~1m 30s left");
    expect(formatEta(120_000)).toBe("~2m left");
  });

  it("switches to hours for the very long jobs", () => {
    expect(formatEta(3_600_000)).toBe("~1h left");
    expect(formatEta(5_400_000)).toBe("~1h 30m left");
  });

  it("never shows a false zero", () => {


    expect(formatEta(200)).toBe("almost done");
    expect(formatEta(0)).toBe("almost done");
  });

  it("rounds to the nearest second rather than truncating", () => {
    expect(formatEta(1_600)).toBe("~2s left");
  });
});

import { describe, expect, it } from "vitest";

import { lineAt } from "./transcript";
import type { Segment } from "./ipc";

const line = (startMs: number, endMs: number): Segment => ({
  startMs,
  endMs,
  text: `${startMs}`,
  speaker: null,
});


const SEGMENTS = [line(0, 2_500), line(4_000, 6_000), line(6_000, 9_000)];

describe("lineAt", () => {
  it("finds the line being spoken", () => {
    expect(lineAt(SEGMENTS, 0)).toBe(0);
    expect(lineAt(SEGMENTS, 1_200)).toBe(0);
    expect(lineAt(SEGMENTS, 5_000)).toBe(1);
    expect(lineAt(SEGMENTS, 8_999)).toBe(2);
  });

  it("holds the last line through a pause instead of dropping the highlight", () => {


    expect(lineAt(SEGMENTS, 3_000)).toBe(0);
    expect(lineAt(SEGMENTS, 20_000)).toBe(2);
  });

  it("reports nothing before the first line has started", () => {
    expect(lineAt([line(1_500, 3_000)], 0)).toBe(-1);
    expect(lineAt([line(1_500, 3_000)], 1_499)).toBe(-1);
    expect(lineAt([line(1_500, 3_000)], 1_500)).toBe(0);
  });

  it("has an answer for a transcript with no lines", () => {
    expect(lineAt([], 4_000)).toBe(-1);
  });

  it("lands on the right line at every boundary", () => {


    const many = Array.from({ length: 50 }, (_, index) => line(index * 100, index * 100 + 50));

    for (let index = 0; index < many.length; index++) {
      expect(lineAt(many, index * 100)).toBe(index);
      expect(lineAt(many, index * 100 + 99)).toBe(index);
    }
  });
});

import { describe, expect, it, vi, afterEach } from "vitest";
import { bytes, clock, count, duration, gpuBackend, relativeTime, timestamp } from "./format";

describe("count", () => {
  it("shows small numbers exactly", () => {
    expect(count(0)).toBe("0");
    expect(count(9_999)).toBe("9,999");
  });


  it("compacts large numbers so columns stay narrow", () => {
    const compact = count(1_500_000);
    expect(compact).not.toBe(count(1_499));
    expect(compact.length).toBeLessThan("1,500,000".length);
    expect(compact).toMatch(/1[.,]5/);
  });
});

describe("clock", () => {
  it("drops the hour field until there is an hour", () => {
    expect(clock(0)).toBe("0:00");
    expect(clock(9_000)).toBe("0:09");
    expect(clock(247_000)).toBe("4:07");
  });

  it("pads minutes and seconds once an hour is showing", () => {
    expect(clock(3_600_000)).toBe("1:00:00");
    expect(clock(3_847_000)).toBe("1:04:07");
  });

  it("treats a negative duration as zero rather than rendering a minus", () => {
    expect(clock(-500)).toBe("0:00");
  });
});

describe("bytes", () => {
  it("uses the unit that keeps the number readable", () => {
    expect(bytes(512)).toBe("512 B");
    expect(bytes(2_048)).toBe("2 KB");
    expect(bytes(5_242_880)).toBe("5.0 MB");
  });

  it("drops the decimal once the number is large enough not to need it", () => {
    expect(bytes(487_601_967)).toBe("465 MB");
  });

  it("switches to gigabytes for a large model", () => {
    expect(bytes(3_095_033_483)).toBe("2.9 GB");
  });
});

describe("duration", () => {
  it("uses milliseconds below a second", () => {
    expect(duration(840)).toBe("840ms");
  });

  it("switches to seconds above one", () => {
    expect(duration(1_240)).toBe("1.2s");
  });
});

describe("relativeTime", () => {
  afterEach(() => vi.useRealTimers());

  it("reports never when there is nothing to report", () => {
    expect(relativeTime(null)).toBe("never");
    expect(relativeTime(undefined)).toBe("never");
  });

  it("treats the last few seconds as now", () => {
    vi.useFakeTimers().setSystemTime(new Date("2026-01-01T12:00:00Z"));
    const tenSecondsAgo = Math.floor(Date.now() / 1000) - 10;
    expect(relativeTime(tenSecondsAgo)).toBe("just now");
  });

  it("scales to the largest sensible unit", () => {
    vi.useFakeTimers().setSystemTime(new Date("2026-01-01T12:00:00Z"));
    const now = Math.floor(Date.now() / 1000);

    expect(relativeTime(now - 300)).toMatch(/5 minutes ago/);
    expect(relativeTime(now - 7_200)).toMatch(/2 hours ago/);
    expect(relativeTime(now - 172_800)).toMatch(/2 days ago/);
  });
});

describe("timestamp", () => {
  it("renders a real local date rather than a raw number", () => {
    const formatted = timestamp(1_767_268_800);
    expect(formatted).not.toMatch(/^\d+$/);
    expect(formatted.length).toBeGreaterThan(8);
  });
});

describe("gpuBackend", () => {
  it("names the backend the platform actually uses", () => {


    expect(gpuBackend("macOs")).toBe("Metal");
    expect(gpuBackend("windows")).toBe("Vulkan");
    expect(gpuBackend("wayland")).toBe("Vulkan");
    expect(gpuBackend("x11")).toBe("Vulkan");
  });

  it("falls back to Vulkan before the scan has answered", () => {
    expect(gpuBackend(null)).toBe("Vulkan");
    expect(gpuBackend(undefined)).toBe("Vulkan");
  });
});

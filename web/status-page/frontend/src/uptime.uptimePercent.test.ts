import { describe, expect, test } from "vitest";
import { uptimePercent } from "./uptime";
import type { OutageWindow } from "./generated/OutageWindow";

const NOW = new Date("2026-07-24T12:00:00+09:00");

describe("uptimePercent", () => {
  test("from == to", () => {
    expect(uptimePercent([], NOW, NOW)).toBeNull();
  });

  test("duration: 1month, downtime: 7min ", () => {
    const windows: OutageWindow[] = [
      {
        startedAt: "2026-08-17T09:00:00+09:00",
        endedAt: "2026-08-17T09:05:00+09:00",
        issueNumber: 73,
      },
      {
        startedAt: "2026-08-17T11:00:00+09:00",
        endedAt: "2026-08-17T11:02:00+09:00",
        issueNumber: 73,
      },
    ];
    const from = new Date("2026-07-24T12:00:00+09:00");
    const to = new Date("2026-08-24T12:00:00+09:00");
    expect(uptimePercent(windows, from, to)).toBeCloseTo(99.984319, 5);
  });
});

import { describe, expect, test } from "vitest";
import { downtimeByDay } from "./uptime";
import type { OutageWindow } from "./generated/OutageWindow";

const NOW = new Date("2026-07-24T12:00:00+09:00");

const byDay = (windows: OutageWindow[], now: Date = NOW) =>
  Object.fromEntries(downtimeByDay(windows, now));

describe("downtimeByDay", () => {
  test("windowが無ければ空", () => {
    expect(byDay([])).toEqual({});
  });

  test("同じ日に収まる window はその日に積む", () => {
    const windows: OutageWindow[] = [
      {
        startedAt: "2026-07-16T10:34:47+09:00",
        endedAt: "2026-07-16T10:39:24+09:00",
        issueNumber: 69,
      },
    ];

    expect(byDay(windows)).toEqual({
      "2026-07-16": { downtimeSeconds: 277, issueNumbers: [69] },
    });
  });

  test("日をまたぐ window は日ごとに分割する", () => {
    const windows: OutageWindow[] = [
      {
        startedAt: "2026-07-22T23:50:00+09:00",
        endedAt: "2026-07-23T00:10:00+09:00",
        issueNumber: 98,
      },
    ];

    expect(byDay(windows)).toEqual({
      "2026-07-22": { downtimeSeconds: 600, issueNumbers: [98] },
      "2026-07-23": { downtimeSeconds: 600, issueNumbers: [98] },
    });
  });

  test("月をまたぐwindowもキーが繰り上がる", () => {
    const windows: OutageWindow[] = [
      {
        startedAt: "2026-07-31T23:55:00+09:00",
        endedAt: "2026-08-01T00:05:00+09:00",
        issueNumber: 120,
      },
    ];

    expect(byDay(windows, new Date("2026-08-01T12:00:00+09:00"))).toEqual({
      "2026-07-31": { downtimeSeconds: 300, issueNumbers: [120] },
      "2026-08-01": { downtimeSeconds: 300, issueNumbers: [120] },
    });
  });

  test("同じ日の複数のwindowは秒数を合算し issue を並べる", () => {
    const windows: OutageWindow[] = [
      {
        startedAt: "2026-07-22T09:00:00+09:00",
        endedAt: "2026-07-22T09:05:00+09:00",
        issueNumber: 98,
      },
      {
        startedAt: "2026-07-22T14:00:00+09:00",
        endedAt: "2026-07-22T14:10:00+09:00",
        issueNumber: 101,
      },
    ];

    expect(byDay(windows)).toEqual({
      "2026-07-22": { downtimeSeconds: 900, issueNumbers: [98, 101] },
    });
  });

  test("同じ issue のwindowが同じ日に複数あっても issue は重複しない", () => {
    const windows: OutageWindow[] = [
      {
        startedAt: "2026-07-17T09:00:00+09:00",
        endedAt: "2026-07-17T09:05:00+09:00",
        issueNumber: 73,
      },
      {
        startedAt: "2026-07-17T11:00:00+09:00",
        endedAt: "2026-07-17T11:02:00+09:00",
        issueNumber: 73,
      },
    ];

    expect(byDay(windows)).toEqual({
      "2026-07-17": { downtimeSeconds: 420, issueNumbers: [73] },
    });
  });

  test("endedAt が無いwindowは now までを積む", () => {
    const windows: OutageWindow[] = [
      {
        startedAt: "2026-07-23T23:30:00+09:00",
        endedAt: null,
        issueNumber: 110,
      },
    ];

    expect(byDay(windows, new Date("2026-07-24T00:30:00+09:00"))).toEqual({
      "2026-07-23": { downtimeSeconds: 1800, issueNumbers: [110] },
      "2026-07-24": { downtimeSeconds: 1800, issueNumbers: [110] },
    });
  });
});

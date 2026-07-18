import type { OutageWindow } from "./generated/OutageWindow";

// 日別バケットの純関数群。「1 日」の境界は訪問者のローカル TZ で切る
// （BFF は ISO 8601 の窓をそのまま返し、日への割り付けはここが担う）。

export type DayInfo = {
  downtimeSeconds: number;
  issueNumbers: number[];
};

/** ローカル TZ の日付キー（例: "2026-07-16"） */
export function localDayKey(d: Date): string {
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

export function startOfLocalDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

/**
 * 窓をローカル TZ の日に割り付け、日ごとの downtime 秒数を積算する。
 * 日をまたぐ窓は日ごとに分割（例: 23:50〜00:10 は前日 10 分 + 当日 10 分）。
 * endedAt が無い窓（障害継続中）は now までとして扱う。
 */
export function downtimeByDay(windows: OutageWindow[], now: Date): Map<string, DayInfo> {
  const map = new Map<string, DayInfo>();
  for (const w of windows) {
    const start = new Date(w.startedAt);
    const end = w.endedAt ? new Date(w.endedAt) : now;
    let day = startOfLocalDay(start);
    while (day.getTime() < end.getTime()) {
      // 翌日 0 時は Date コンストラクタに任せる（+24h 固定だと DST 切替日にずれる）
      const next = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1);
      const overlap =
        Math.min(end.getTime(), next.getTime()) - Math.max(start.getTime(), day.getTime());
      if (overlap > 0) {
        const key = localDayKey(day);
        const info = map.get(key) ?? { downtimeSeconds: 0, issueNumbers: [] };
        info.downtimeSeconds += overlap / 1000;
        if (!info.issueNumbers.includes(w.issueNumber)) info.issueNumbers.push(w.issueNumber);
        map.set(key, info);
      }
      day = next;
    }
  }
  return map;
}

/**
 * [from, now] の観測期間に対する uptime %。
 * 観測期間の起点は「観測なし」の日を分母に入れないよう呼び出し側で
 * max(表示範囲の開始, since) にクランプして渡す。
 */
export function uptimePercent(windows: OutageWindow[], from: Date, now: Date): number | null {
  const total = now.getTime() - from.getTime();
  if (total <= 0) return null;
  let down = 0;
  for (const w of windows) {
    const start = Math.max(new Date(w.startedAt).getTime(), from.getTime());
    const end = Math.min(
      (w.endedAt ? new Date(w.endedAt) : now).getTime(),
      now.getTime(),
    );
    if (end > start) down += end - start;
  }
  return 100 * (1 - down / total);
}

export type DayState = "ok" | "partial" | "major" | "nodata" | "future";

const MAJOR_THRESHOLD_SECONDS = 3600;

/** 色の閾値: 緑 = 0 / 黄 = 1h 未満 / 赤 = 1h 以上 / 灰 = 観測なし（since 前・未来） */
export function dayState(
  day: Date,
  info: DayInfo | undefined,
  since: Date,
  now: Date,
): DayState {
  if (day.getTime() > now.getTime()) return "future";
  if (day.getTime() < startOfLocalDay(since).getTime()) return "nodata";
  const seconds = info?.downtimeSeconds ?? 0;
  if (seconds >= MAJOR_THRESHOLD_SECONDS) return "major";
  if (seconds > 0) return "partial";
  return "ok";
}

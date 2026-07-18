import { useEffect, useMemo, useState } from "react";
import type { UptimeResponse } from "./generated/UptimeResponse";
import {
  dayState,
  downtimeByDay,
  localDayKey,
  startOfLocalDay,
  uptimePercent,
  type DayInfo,
  type DayState,
} from "./uptime";

const MONTHS_SHOWN = 3;
const WEEKDAY_INITIALS = ["S", "M", "T", "W", "T", "F", "S"];

const STATE_LABEL: Record<DayState, string> = {
  ok: "operational",
  partial: "partial outage (< 1 h)",
  major: "major outage (≥ 1 h)",
  nodata: "no data",
  future: "",
};

function useUptime() {
  const [data, setData] = useState<UptimeResponse | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await fetch("/api/uptime");
        if (!res.ok) throw new Error(`http ${res.status}`);
        const body: UptimeResponse = await res.json();
        if (!cancelled) setData(body);
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return { data, failed };
}

type SelectedDay = { date: Date; state: DayState; info: DayInfo | undefined };

function MonthCalendar({
  first,
  byDay,
  since,
  now,
  onSelect,
}: {
  first: Date; // 月初日（ローカル TZ）
  byDay: Map<string, DayInfo>;
  since: Date;
  now: Date;
  onSelect: (day: SelectedDay) => void;
}) {
  const monthName = first.toLocaleDateString("en-US", { month: "long", year: "numeric" });
  const daysInMonth = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate();
  const cells: (Date | null)[] = [
    ...Array<null>(first.getDay()).fill(null),
    ...Array.from({ length: daysInMonth }, (_, i) =>
      new Date(first.getFullYear(), first.getMonth(), i + 1),
    ),
  ];

  return (
    <div className="cal-month">
      <div className="cal-title">{monthName}</div>
      <div className="cal-grid">
        {WEEKDAY_INITIALS.map((w, i) => (
          <div key={`w${i}`} className="cal-weekday">
            {w}
          </div>
        ))}
        {cells.map((day, i) => {
          if (!day) return <div key={`b${i}`} />;
          const info = byDay.get(localDayKey(day));
          const state = dayState(day, info, since, now);
          return (
            <button
              key={localDayKey(day)}
              type="button"
              className={`cal-day ${state}`}
              aria-label={`${localDayKey(day)}: ${STATE_LABEL[state] || "future"}`}
              disabled={state === "future"}
              onMouseEnter={() => onSelect({ date: day, state, info })}
              onFocus={() => onSelect({ date: day, state, info })}
              onClick={() => onSelect({ date: day, state, info })}
            >
              {day.getDate()}
            </button>
          );
        })}
      </div>
    </div>
  );
}

function DayDetail({ selected, repoUrl }: { selected: SelectedDay | null; repoUrl: string }) {
  if (!selected) {
    return <div className="cal-detail dim-text">Hover or tap a day for details.</div>;
  }
  const { date, state, info } = selected;
  const dateLabel = date.toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
  if (state === "nodata") {
    return (
      <div className="cal-detail">
        <strong>{dateLabel}</strong> — no data (before external probing began)
      </div>
    );
  }
  const seconds = info?.downtimeSeconds ?? 0;
  return (
    <div className="cal-detail">
      <strong>{dateLabel}</strong> —{" "}
      {seconds === 0 ? "no downtime recorded" : `${Math.round(seconds / 60)} min downtime`}
      {info && info.issueNumbers.length > 0 && (
        <>
          {" · "}
          {info.issueNumbers.map((n, i) => (
            <span key={n}>
              {i > 0 && ", "}
              <a href={`${repoUrl}/issues/${n}`}>#{n}</a>
            </span>
          ))}
        </>
      )}
    </div>
  );
}

export default function UptimePage({ repoUrl }: { repoUrl: string }) {
  const { data, failed } = useUptime();
  const [selected, setSelected] = useState<SelectedDay | null>(null);
  // マウント時点の「今」で固定（fetch も mount 時 1 回。レンダーごとに動かさない）
  const [now] = useState(() => new Date());

  const months = useMemo(() => {
    return Array.from({ length: MONTHS_SHOWN }, (_, i) => {
      const offset = MONTHS_SHOWN - 1 - i; // 古い月が左
      return new Date(now.getFullYear(), now.getMonth() - offset, 1);
    });
  }, [now]);

  const byDay = useMemo(
    () => downtimeByDay(data?.windows ?? [], now),
    [data, now],
  );

  if (failed) {
    return <p className="section-note">Failed to load uptime data.</p>;
  }
  if (!data) {
    return <p className="section-note">Loading…</p>;
  }

  const since = new Date(data.since);
  // 「観測なし」の日を分母に入れない: 表示範囲の開始と since の遅い方から数える
  const observedFrom = new Date(
    Math.max(months[0].getTime(), startOfLocalDay(since).getTime()),
  );
  const percent = uptimePercent(data.windows, observedFrom, now);

  return (
    <>
      <section>
        <h2>Uptime</h2>
        <p className="section-note">
          Daily availability of <strong>bons8i.hagaspa.com</strong> as seen by an external
          probe (AWS Lambda, every 10 minutes). Outage windows are recorded as{" "}
          <a href={`${repoUrl}/issues?q=label%3Aoutage`}>GitHub Issues</a> — open means an
          outage is ongoing.
        </p>
        {percent != null && (
          <div className="uptime-summary">
            <span className="uptime-percent">{percent.toFixed(2)}%</span> uptime since{" "}
            {observedFrom.toLocaleDateString("en-US", {
              year: "numeric",
              month: "short",
              day: "numeric",
            })}
          </div>
        )}
        <div className="cal-row">
          {months.map((first) => (
            <MonthCalendar
              key={localDayKey(first)}
              first={first}
              byDay={byDay}
              since={since}
              now={now}
              onSelect={setSelected}
            />
          ))}
        </div>
        <DayDetail selected={selected} repoUrl={repoUrl} />
        <div className="cal-legend">
          <span>
            <i className="cal-dot ok" /> operational
          </span>
          <span>
            <i className="cal-dot partial" /> partial (&lt; 1 h)
          </span>
          <span>
            <i className="cal-dot major" /> major (≥ 1 h)
          </span>
          <span>
            <i className="cal-dot nodata" /> no data
          </span>
        </div>
      </section>
    </>
  );
}

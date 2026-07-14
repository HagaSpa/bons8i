import { useEffect, useState } from "react";
import type { StatusResponse } from "./generated/StatusResponse";
import type { FiringAlert } from "./generated/FiringAlert";

// BFF のクラスタキャッシュ TTL と同周期
const REFRESH_MS = 60_000;

const REPO_URL = "https://github.com/HagaSpa/bons8i";

function useStatus() {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [unreachable, setUnreachable] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const res = await fetch("/api/status");
        if (!res.ok) throw new Error(`http ${res.status}`);
        const data: StatusResponse = await res.json();
        if (!cancelled) {
          setStatus(data);
          setUnreachable(false);
        }
      } catch {
        if (!cancelled) setUnreachable(true);
      }
    };
    load();
    const id = setInterval(load, REFRESH_MS);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  return { status, unreachable };
}

type BadgeKind = StatusResponse["overall"] | "loading" | "unreachable";

const BADGE: Record<BadgeKind, { label: string; className: string }> = {
  operational: { label: "All Systems Operational", className: "ok" },
  degraded: { label: "Active Alerts", className: "bad" },
  unknown: { label: "Status Unknown", className: "dim" },
  loading: { label: "Loading…", className: "dim" },
  unreachable: { label: "Status API Unreachable", className: "dim" },
};

const fmt = {
  num: (v: number | null, digits: number, unit: string) =>
    v == null ? "–" : `${v.toFixed(digits)}${unit}`,
  uptime(v: number | null) {
    if (v == null) return "–";
    const d = Math.floor(v / 86_400);
    const h = Math.floor((v % 86_400) / 3_600);
    return d > 0 ? `${d}d ${h}h` : `${h}h ${Math.floor((v % 3_600) / 60)}m`;
  },
  hours(v: number | null) {
    if (v == null) return "–";
    return v < 1 ? `${Math.round(v * 60)} min` : `${v.toFixed(1)} h`;
  },
  time: (iso: string) => new Date(iso).toLocaleTimeString(),
};

function Card({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="card">
      <div className="card-label">{label}</div>
      <div className="card-value">{value}</div>
      {hint && <div className="card-hint">{hint}</div>}
    </div>
  );
}

function AlertRow({ alert }: { alert: FiringAlert }) {
  return (
    <li className="alert-row">
      <span className={`severity ${alert.severity ?? "none"}`}>
        {alert.severity ?? "unknown"}
      </span>
      <span className="alert-name">{alert.name}</span>
      {alert.summary && <span className="alert-summary">{alert.summary}</span>}
      {alert.startedAt && (
        <span className="alert-since">
          since {new Date(alert.startedAt).toLocaleString()}
        </span>
      )}
    </li>
  );
}

export default function App() {
  const { status, unreachable } = useStatus();
  const badgeKind: BadgeKind = unreachable
    ? "unreachable"
    : (status?.overall ?? "loading");
  const badge = BADGE[badgeKind];
  const m = status?.metrics;
  const issues = status?.issues ?? null;

  return (
    <main>
      <header>
        <h1>🪴 bons8i status</h1>
        <p className="subtitle">
          Live status of a single-node Raspberry Pi 5 Kubernetes cluster
          (kubeadm), reconciled by ArgoCD from{" "}
          <a href={REPO_URL}>HagaSpa/bons8i</a>.
        </p>
      </header>

      <div className={`badge ${badge.className}`}>
        <span className="badge-dot" />
        {badge.label}
      </div>

      {status && status.firingAlerts.length > 0 && (
        <section>
          <h2>Firing alerts</h2>
          <ul className="alerts">
            {status.firingAlerts.map((a) => (
              <AlertRow key={`${a.name}-${a.startedAt}`} alert={a} />
            ))}
          </ul>
        </section>
      )}

      <section>
        <h2>Cluster</h2>
        <div className="grid">
          <Card label="Node temperature" value={fmt.num(m?.nodeTempCelsius ?? null, 1, " °C")} />
          <Card label="CPU usage" value={fmt.num(m?.cpuUsagePercent ?? null, 1, " %")} />
          <Card label="Memory usage" value={fmt.num(m?.memoryUsagePercent ?? null, 1, " %")} />
          <Card label="Uptime" value={fmt.uptime(m?.uptimeSeconds ?? null)} />
          <Card label="Running pods" value={fmt.num(m?.runningPods ?? null, 0, "")} />
        </div>
      </section>

      <section>
        <h2>Alert history</h2>
        <p className="section-note">
          Every firing alert is filed as a{" "}
          <a href={`${REPO_URL}/issues?q=is%3Aissue`}>GitHub Issue</a> and
          auto-closed when it resolves — an open issue means an incident is
          ongoing.
        </p>
        <div className="grid">
          <Card label="Open issues" value={issues ? String(issues.openCount) : "–"} />
          <Card
            label="Closed (30 days)"
            value={issues ? String(issues.closedCount30d) : "–"}
          />
          <Card
            label="Avg. time to close"
            value={fmt.hours(issues?.avgHoursToClose30d ?? null)}
            hint="last 30 days"
          />
        </div>
      </section>

      <footer>
        {status && <span>Updated {fmt.time(status.generatedAt)} · refreshes every 60 s</span>}
        <span>
          Built with Rust + React · Source on{" "}
          <a href={`${REPO_URL}/tree/main/web/status-page`}>GitHub</a>
        </span>
      </footer>
    </main>
  );
}

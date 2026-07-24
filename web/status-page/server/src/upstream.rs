use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

use crate::types::{FiringAlert, IssueStats, MetricCards, OutageWindow};

// クエリはコードに固定。ユーザー入力は上流に一切届かない
const Q_NODE_TEMP: &str = "max(node_hwmon_temp_celsius)";
const Q_CPU_PERCENT: &str = r#"100 * (1 - avg(rate(node_cpu_seconds_total{mode="idle"}[5m])))"#;
const Q_MEM_PERCENT: &str =
    "100 * (1 - node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes)";
const Q_UPTIME_SECONDS: &str = "time() - max(node_boot_time_seconds)";
const Q_RUNNING_PODS: &str = r#"sum(kube_pod_status_phase{phase="Running"})"#;

#[derive(Deserialize)]
struct PromResponse {
    data: PromData,
}

#[derive(Deserialize)]
struct PromData {
    result: Vec<PromResult>,
}

#[derive(Deserialize)]
struct PromResult {
    // [unix_timestamp, "値の文字列"]
    value: (f64, String),
}

async fn query_vm(client: &reqwest::Client, base: &str, query: &str) -> Option<f64> {
    let url = format!("{base}/api/v1/query");
    let resp = client
        .get(&url)
        .query(&[("query", query)])
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| tracing::warn!(query, error = %e, "VM query failed"))
        .ok()?;
    let body: PromResponse = resp
        .json()
        .await
        .map_err(|e| tracing::warn!(query, error = %e, "VM response parse failed"))
        .ok()?;
    body.data.result.first()?.value.1.parse().ok()
}

pub async fn fetch_metrics(client: &reqwest::Client, vm_base: &str) -> MetricCards {
    let (node_temp_celsius, cpu_usage_percent, memory_usage_percent, uptime_seconds, running_pods) = tokio::join!(
        query_vm(client, vm_base, Q_NODE_TEMP),
        query_vm(client, vm_base, Q_CPU_PERCENT),
        query_vm(client, vm_base, Q_MEM_PERCENT),
        query_vm(client, vm_base, Q_UPTIME_SECONDS),
        query_vm(client, vm_base, Q_RUNNING_PODS),
    );
    MetricCards {
        node_temp_celsius,
        cpu_usage_percent,
        memory_usage_percent,
        uptime_seconds,
        running_pods,
    }
}

#[derive(Deserialize)]
struct AmAlert {
    labels: std::collections::HashMap<String, String>,
    annotations: std::collections::HashMap<String, String>,
    #[serde(rename = "startsAt")]
    starts_at: Option<String>,
}

/// Watchdog は「常時発火が正常」の死活監視用アラートなので除外する。
pub async fn fetch_alerts(
    client: &reqwest::Client,
    am_base: &str,
) -> Result<Vec<FiringAlert>, reqwest::Error> {
    let url = format!("{am_base}/api/v2/alerts");
    let alerts: Vec<AmAlert> = client
        .get(&url)
        .query(&[
            ("active", "true"),
            ("silenced", "false"),
            ("inhibited", "false"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(alerts
        .into_iter()
        .filter(|a| a.labels.get("alertname").map(String::as_str) != Some("Watchdog"))
        .map(|a| FiringAlert {
            name: a
                .labels
                .get("alertname")
                .cloned()
                .unwrap_or_else(|| "unknown".into()),
            severity: a.labels.get("severity").cloned(),
            summary: a
                .annotations
                .get("summary")
                .or_else(|| a.annotations.get("description"))
                .cloned(),
            started_at: a.starts_at,
        })
        .collect())
}

#[derive(Clone, Deserialize)]
pub struct GhIssue {
    number: u32,
    state: String,
    created_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    labels: Vec<GhLabel>,
    // PR も issues エンドポイントに混ざって返る。このキーの有無で除外する
    pull_request: Option<serde_json::Value>,
}

#[derive(Clone, Deserialize)]
struct GhLabel {
    name: String,
}

/// alert ラベル付き Issue の一覧。Issue 統計と uptime calendar の両方が
/// この 1 レスポンスから導出される（キャッシュも共有 = GitHub へのリクエストは増えない）。
/// 無認証（public データのみ）。60 req/h per IP の制限は呼び出し元のキャッシュで吸収する。
pub async fn fetch_issues(
    client: &reqwest::Client,
    repo: &str,
) -> Result<Vec<GhIssue>, reqwest::Error> {
    let url = format!("https://api.github.com/repos/{repo}/issues");
    let issues: Vec<GhIssue> = client
        .get(&url)
        .header("User-Agent", "bons8i-status-page")
        .header("Accept", "application/vnd.github+json")
        .query(&[
            ("state", "all"),
            // ATG が付与する alert ラベルで絞る。Renovate の Dependency Dashboard 等の
            // アラート以外の Issue を統計から除外する
            ("labels", "alert"),
            ("per_page", "100"),
            ("sort", "created"),
            ("direction", "desc"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(issues
        .into_iter()
        .filter(|i| i.pull_request.is_none())
        .collect())
}

pub fn issue_stats(issues: &[GhIssue]) -> IssueStats {
    let cutoff = Utc::now() - Duration::days(30);
    let open_count = issues.iter().filter(|i| i.state == "open").count() as u32;
    let closed_30d: Vec<_> = issues
        .iter()
        .filter(|i| i.closed_at.is_some_and(|c| c > cutoff))
        .collect();
    let avg_hours = (!closed_30d.is_empty()).then(|| {
        let total_hours: f64 = closed_30d
            .iter()
            .map(|i| (i.closed_at.unwrap() - i.created_at).num_seconds() as f64 / 3600.0)
            .sum();
        total_hours / closed_30d.len() as f64
    });

    IssueStats {
        open_count,
        closed_count_30d: closed_30d.len() as u32,
        avg_hours_to_close_30d: avg_hours,
    }
}

/// outage ラベル付き Issue だけが「訪問者視点の停止」の記録（ラベル 2 層化）。
/// 運用アラート（alert のみ）はサービスとしては生きているので窓にしない。
pub fn outage_windows(issues: &[GhIssue]) -> Vec<OutageWindow> {
    issues
        .iter()
        .filter(|i| i.labels.iter().any(|l| l.name == "outage"))
        .map(|i| OutageWindow {
            started_at: i.created_at.to_rfc3339(),
            ended_at: i.closed_at.map(|c| c.to_rfc3339()),
            issue_number: i.number,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(number: u32, labels: &[&str], created_at: &str, closed_at: Option<&str>) -> GhIssue {
        GhIssue {
            number,
            state: if closed_at.is_some() {
                "closed"
            } else {
                "open"
            }
            .into(),
            created_at: created_at.parse().unwrap(),
            closed_at: closed_at.map(|c| c.parse().unwrap()),
            labels: labels
                .iter()
                .map(|n| GhLabel { name: (*n).into() })
                .collect(),
            pull_request: None,
        }
    }

    #[test]
    fn outage_windows_filters_by_label() {
        let issues = vec![
            issue(
                69,
                &["alert", "outage"],
                "2026-07-16T10:34:47Z",
                Some("2026-07-16T10:39:24Z"),
            ),
            issue(
                65,
                &["alert"],
                "2026-07-16T11:05:00Z",
                Some("2026-07-16T11:15:00Z"),
            ),
            issue(90, &["alert", "outage"], "2026-07-18T00:00:00Z", None),
        ];
        let windows = outage_windows(&issues);
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].issue_number, 69);
        assert_eq!(windows[0].started_at, "2026-07-16T10:34:47+00:00");
        assert_eq!(
            windows[0].ended_at.as_deref(),
            Some("2026-07-16T10:39:24+00:00")
        );
        // open 中の Issue は ended_at が無い = 障害継続中
        assert_eq!(windows[1].issue_number, 90);
        assert_eq!(windows[1].ended_at, None);
    }

    #[test]
    fn issue_stats_counts_open_and_recent_closes() {
        let now = Utc::now();
        let recent = (now - Duration::hours(2)).to_rfc3339();
        let recent_close = (now - Duration::hours(1)).to_rfc3339();
        let old = (now - Duration::days(90)).to_rfc3339();
        let old_close = (now - Duration::days(89)).to_rfc3339();
        let issues = vec![
            issue(1, &["alert"], &recent, None),
            issue(2, &["alert"], &recent, Some(&recent_close)),
            issue(3, &["alert"], &old, Some(&old_close)), // 30 日窓の外
        ];
        let stats = issue_stats(&issues);
        assert_eq!(stats.open_count, 1);
        assert_eq!(stats.closed_count_30d, 1);
        assert!((stats.avg_hours_to_close_30d.unwrap() - 1.0).abs() < 0.1);
    }
}

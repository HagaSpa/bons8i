use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

use crate::types::{FiringAlert, IssueStats, MetricCards};

/// クエリはすべてこのコードに固定。ユーザー入力は一切受けない（BFF の設計原則）。
const Q_NODE_TEMP: &str = "max(node_hwmon_temp_celsius)";
const Q_CPU_PERCENT: &str =
    r#"100 * (1 - avg(rate(node_cpu_seconds_total{mode="idle"}[5m])))"#;
const Q_MEM_PERCENT: &str =
    "100 * (1 - node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes)";
const Q_UPTIME_SECONDS: &str = "time() - max(node_boot_time_seconds)";
const Q_RUNNING_PODS: &str = r#"sum(kube_pod_status_phase{phase="Running"})"#;

/// Prometheus 互換 query API のレスポンス（必要な部分だけデシリアライズ）
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
    /// [unix_timestamp, "値の文字列"] の 2 要素
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

/// Alertmanager API v2 の alert オブジェクト（必要な部分だけ）
#[derive(Deserialize)]
struct AmAlert {
    labels: std::collections::HashMap<String, String>,
    annotations: std::collections::HashMap<String, String>,
    #[serde(rename = "startsAt")]
    starts_at: Option<String>,
}

/// 発火中アラートの取得。Watchdog は「常時発火が正常」の死活監視用アラートなので除外する。
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

/// GitHub Issues API の issue オブジェクト（必要な部分だけ）
#[derive(Deserialize)]
struct GhIssue {
    state: String,
    created_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    /// PR は issues エンドポイントに混ざって返るのでこのキーの有無で除外する
    pull_request: Option<serde_json::Value>,
}

/// public リポの公開データのみ・無認証（60 req/h per IP、呼び出し元の 5 分キャッシュで最大 12 req/h）。
pub async fn fetch_issue_stats(
    client: &reqwest::Client,
    repo: &str,
) -> Result<IssueStats, reqwest::Error> {
    let url = format!("https://api.github.com/repos/{repo}/issues");
    let issues: Vec<GhIssue> = client
        .get(&url)
        .header("User-Agent", "bons8i-status-page")
        .header("Accept", "application/vnd.github+json")
        .query(&[
            ("state", "all"),
            ("per_page", "100"),
            ("sort", "created"),
            ("direction", "desc"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let cutoff = Utc::now() - Duration::days(30);
    let issues: Vec<_> = issues
        .into_iter()
        .filter(|i| i.pull_request.is_none())
        .collect();

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

    Ok(IssueStats {
        open_count,
        closed_count_30d: closed_30d.len() as u32,
        avg_hours_to_close_30d: avg_hours,
    })
}

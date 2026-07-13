use serde::Serialize;
use ts_rs::TS;

/// `/api/status` のレスポンス。フロントの型は ts-rs がここから生成する
/// （`cargo test` 時に `frontend/src/generated/` へ出力）。
#[derive(Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/generated/")]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub overall: Overall,
    pub firing_alerts: Vec<FiringAlert>,
    pub metrics: MetricCards,
    /// GitHub API の取得失敗時は null（ページ全体は生かす）
    pub issues: Option<IssueStats>,
    pub generated_at: String,
}

#[derive(Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/generated/")]
#[serde(rename_all = "lowercase")]
pub enum Overall {
    /// 発火中アラート 0（Watchdog 除外後）
    Operational,
    /// 発火中アラートあり
    Degraded,
    /// Alertmanager に到達できず判定不能
    Unknown,
}

#[derive(Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/generated/")]
#[serde(rename_all = "camelCase")]
pub struct FiringAlert {
    pub name: String,
    pub severity: Option<String>,
    pub summary: Option<String>,
    pub started_at: Option<String>,
}

/// 各カードは Option — 個別クエリの失敗でページ全体を 500 にしない。
#[derive(Clone, Default, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/generated/")]
#[serde(rename_all = "camelCase")]
pub struct MetricCards {
    pub node_temp_celsius: Option<f64>,
    pub cpu_usage_percent: Option<f64>,
    pub memory_usage_percent: Option<f64>,
    pub uptime_seconds: Option<f64>,
    pub running_pods: Option<f64>,
}

#[derive(Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/generated/")]
#[serde(rename_all = "camelCase")]
pub struct IssueStats {
    pub open_count: u64,
    pub closed_count_30d: u64,
    /// 直近 30 日にクローズされた issue の平均クローズ時間（時間単位）
    pub avg_hours_to_close_30d: Option<f64>,
}

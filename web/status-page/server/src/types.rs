use serde::Serialize;
use ts_rs::TS;

#[derive(Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/generated/")]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub overall: Overall,
    pub firing_alerts: Vec<FiringAlert>,
    pub metrics: MetricCards,
    pub issues: Option<IssueStats>,
    pub generated_at: String,
}

#[derive(Clone, Copy, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/generated/")]
#[serde(rename_all = "lowercase")]
pub enum Overall {
    Operational,
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
pub struct UptimeResponse {
    pub windows: Vec<OutageWindow>,
    /// 外形監視の記録開始時刻（ISO 8601）。これより前の日は「観測なし」として描く
    pub since: String,
    pub generated_at: String,
}

/// outage ラベル付き Issue 1 件 = 訪問者視点の downtime 窓 1 つ
#[derive(Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/generated/")]
#[serde(rename_all = "camelCase")]
pub struct OutageWindow {
    pub started_at: String,
    /// None = 障害継続中
    pub ended_at: Option<String>,
    pub issue_number: u32,
}

#[derive(Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/generated/")]
#[serde(rename_all = "camelCase")]
pub struct IssueStats {
    // u32 なのは ts-rs の写像のため（u64 は bigint になるが JSON.parse は number を返す）
    pub open_count: u32,
    pub closed_count_30d: u32,
    pub avg_hours_to_close_30d: Option<f64>,
}

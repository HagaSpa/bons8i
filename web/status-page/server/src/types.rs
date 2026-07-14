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
pub struct IssueStats {
    // u32 なのは ts-rs の写像のため（u64 は bigint になるが JSON.parse は number を返す）
    pub open_count: u32,
    pub closed_count_30d: u32,
    pub avg_hours_to_close_30d: Option<f64>,
}

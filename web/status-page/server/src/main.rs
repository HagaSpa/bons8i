mod cache;
mod types;
mod upstream;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::{Html, Json};
use axum::routing::get;
use axum::Router;

use cache::Cache;
use types::{FiringAlert, MetricCards, Overall, StatusResponse};

/// クラスタ内上流（VM / Alertmanager）のキャッシュ TTL
const CLUSTER_TTL: Duration = Duration::from_secs(60);
/// GitHub API のキャッシュ TTL（無認証 60 req/h 制限に対し最大 12 req/h に抑える）
const GITHUB_TTL: Duration = Duration::from_secs(300);

struct AppState {
    client: reqwest::Client,
    vm_url: String,
    am_url: String,
    github_repo: String,
    /// アラートとメトリクスをまとめて 1 エントリでキャッシュ
    cluster_cache: Cache<(Option<Vec<FiringAlert>>, MetricCards)>,
    issue_cache: Cache<types::IssueStats>,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "status_page=info".into()),
        )
        .init();

    let state = Arc::new(AppState {
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client"),
        // ローカル開発は kubectl port-forward 前提のデフォルト。本番はマニフェストの env で Service 名を注入
        vm_url: env_or("VM_URL", "http://127.0.0.1:8428"),
        am_url: env_or("ALERTMANAGER_URL", "http://127.0.0.1:9093"),
        github_repo: env_or("GITHUB_REPO", "HagaSpa/bons8i"),
        cluster_cache: Cache::new(CLUSTER_TTL),
        issue_cache: Cache::new(GITHUB_TTL),
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/healthz", get(|| async { "ok" }))
        .route("/api/status", get(api_status))
        .with_state(state.clone());

    let addr = env_or("LISTEN_ADDR", "0.0.0.0:8080");
    tracing::info!(addr, vm = state.vm_url, am = state.am_url, "starting");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}

/// Day 2 で React ビルド成果物（rust-embed）に置き換えるまでの暫定トップページ
async fn index() -> Html<&'static str> {
    Html("<!doctype html><title>bons8i status</title><p>bons8i status — frontend coming soon. See <a href=\"/api/status\">/api/status</a>.")
}

async fn api_status(State(state): State<Arc<AppState>>) -> Json<StatusResponse> {
    let cluster = state
        .cluster_cache
        .get_or_refresh(|| async {
            let (alerts, metrics) = tokio::join!(
                upstream::fetch_alerts(&state.client, &state.am_url),
                upstream::fetch_metrics(&state.client, &state.vm_url),
            );
            let alerts = match alerts {
                Ok(a) => Some(a),
                Err(e) => {
                    tracing::warn!(error = %e, "alertmanager fetch failed");
                    None
                }
            };
            Some((alerts, metrics))
        })
        .await;

    let issues = state
        .issue_cache
        .get_or_refresh(|| async {
            upstream::fetch_issue_stats(&state.client, &state.github_repo)
                .await
                .map_err(|e| tracing::warn!(error = %e, "github fetch failed"))
                .ok()
        })
        .await;

    let (alerts, metrics) = cluster.unwrap_or((None, MetricCards::default()));
    let (overall, firing_alerts) = match alerts {
        Some(list) if list.is_empty() => (Overall::Operational, list),
        Some(list) => (Overall::Degraded, list),
        None => (Overall::Unknown, Vec::new()),
    };

    Json(StatusResponse {
        overall,
        firing_alerts,
        metrics,
        issues,
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

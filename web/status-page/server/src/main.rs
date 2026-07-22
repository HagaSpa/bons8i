mod cache;
mod types;
mod upstream;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use rust_embed::RustEmbed;

use cache::Cache;
use tokio::signal;
use types::{FiringAlert, MetricCards, Overall, StatusResponse, UptimeResponse};

// コンパイルには frontend/dist/ の実在が必要（先に `npm run build` を実行しておく）
#[derive(RustEmbed)]
#[folder = "../frontend/dist/"]
struct Assets;

const CLUSTER_TTL: Duration = Duration::from_secs(60);
// GitHub は無認証 60 req/h 制限があるので長めに
const GITHUB_TTL: Duration = Duration::from_secs(300);
// 外形監視（Lambda probe）が outage Issue の記録を始めた時刻。これより前は「観測なし」
const PROBE_SINCE: &str = "2026-07-16T00:00:00Z";
// SPA のクライアントルート。ここに載せたパスだけ index.html を返す
// （ワイルドカード fallback にはせず、未知パスへの 404 を保つ）
const SPA_ROUTES: &[&str] = &["uptime"];

struct AppState {
    client: reqwest::Client,
    vm_url: String,
    am_url: String,
    github_repo: String,
    cluster_cache: Cache<(Option<Vec<FiringAlert>>, MetricCards)>,
    // 生の Issue リストを持ち、統計（/api/status）と outage 窓（/api/uptime）を
    // 同じレスポンスから導出する = GitHub へのリクエストは 1 本のまま
    issue_cache: Cache<Vec<upstream::GhIssue>>,
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
            // reqwest 0.13 の既定は OS の証明書ストアを読む platform-verifier だが、
            // scratch イメージにはストアが無いため、同梱した Mozilla ルートだけで検証する
            .tls_certs_only(
                webpki_root_certs::TLS_SERVER_ROOT_CERTS
                    .iter()
                    .map(|der| reqwest::Certificate::from_der(der).expect("bundled webpki root"))
                    .collect::<Vec<_>>(),
            )
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client"),
        // デフォルトはローカル開発用（kubectl port-forward 前提）。本番は env で Service 名を注入
        vm_url: env_or("VM_URL", "http://127.0.0.1:8428"),
        am_url: env_or("ALERTMANAGER_URL", "http://127.0.0.1:9093"),
        github_repo: env_or("GITHUB_REPO", "HagaSpa/bons8i"),
        cluster_cache: Cache::new(CLUSTER_TTL),
        issue_cache: Cache::new(GITHUB_TTL),
    });

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/api/status", get(api_status))
        .route("/api/uptime", get(api_uptime))
        .fallback(static_handler)
        .with_state(state.clone());

    let addr = env_or("LISTEN_ADDR", "0.0.0.0:8080");
    tracing::info!(addr, vm = state.vm_url, am = state.am_url, "starting");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server");
}

// シャットダウンを待機する非同期関数
// https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs#L54-L76
async fn shutdown_signal() {
    let sig_int = async {
        signal::ctrl_c()
            .await
            .expect("failed to prepare Ctrl+C handler")
    };
    let sig_term = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to prepare terminate signal handler")
            .recv()
            .await
    };
    tokio::select! {
        _ = sig_int => {
            tracing::info!("SIGINT received, starting graceful shutdown")
        },
        _ = sig_term => {
            tracing::info!("SIGTERM received, starting graceful shutdown")
        },
    }
}

/// 同梱した React 成果物の配信。Vite の assets/ はファイル名にハッシュが入るので
/// immutable、index.html は no-cache（デプロイ即反映）。
async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() || SPA_ROUTES.contains(&path) {
        "index.html"
    } else {
        path
    };
    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let cache = if path.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            (
                [(CONTENT_TYPE, mime.as_ref()), (CACHE_CONTROL, cache)],
                file.data,
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
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

    let issues = cached_issues(&state)
        .await
        .map(|list| upstream::issue_stats(&list));

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

async fn api_uptime(State(state): State<Arc<AppState>>) -> Json<UptimeResponse> {
    let windows = cached_issues(&state)
        .await
        .map(|list| upstream::outage_windows(&list))
        .unwrap_or_default();
    Json(UptimeResponse {
        windows,
        since: PROBE_SINCE.into(),
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

async fn cached_issues(state: &AppState) -> Option<Vec<upstream::GhIssue>> {
    state
        .issue_cache
        .get_or_refresh(|| async {
            upstream::fetch_issues(&state.client, &state.github_repo)
                .await
                .map_err(|e| tracing::warn!(error = %e, "github fetch failed"))
                .ok()
        })
        .await
}

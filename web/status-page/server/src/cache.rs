use std::future::Future;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

/// TTL 付きキャッシュ。リフレッシュ中はロックを保持する（single-flight =
/// 同時アクセスが殺到しても上流へのリクエストは 1 本）。リフレッシュ失敗時は
/// 期限切れでも最後の成功値を返す。
pub struct Cache<T> {
    ttl: Duration,
    inner: Mutex<Option<Entry<T>>>,
}

struct Entry<T> {
    fetched_at: Instant,
    value: T,
}

impl<T: Clone> Cache<T> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            inner: Mutex::new(None),
        }
    }

    pub async fn get_or_refresh<F, Fut>(&self, refresh: F) -> Option<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<T>>,
    {
        let mut guard = self.inner.lock().await;
        if let Some(entry) = guard.as_ref() {
            if entry.fetched_at.elapsed() < self.ttl {
                return Some(entry.value.clone());
            }
        }
        match refresh().await {
            Some(value) => {
                *guard = Some(Entry {
                    fetched_at: Instant::now(),
                    value: value.clone(),
                });
                Some(value)
            }
            None => guard.as_ref().map(|e| e.value.clone()),
        }
    }
}

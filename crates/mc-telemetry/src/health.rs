use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

#[async_trait]
pub trait ReadinessCheck: Send + Sync {
    fn name(&self) -> &str;
    async fn check(&self) -> Result<(), String>;
}

struct FnCheck<F> {
    name: String,
    check: F,
}

#[async_trait]
impl<F, Fut> ReadinessCheck for FnCheck<F>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self) -> Result<(), String> {
        (self.check)().await
    }
}

pub fn check_fn<F, Fut>(name: impl Into<String>, check: F) -> Arc<dyn ReadinessCheck>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    Arc::new(FnCheck {
        name: name.into(),
        check,
    })
}

#[derive(Clone, Default)]
pub struct Health {
    checks: Arc<Vec<Arc<dyn ReadinessCheck>>>,
}

impl Health {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_check(mut self, check: Arc<dyn ReadinessCheck>) -> Self {
        Arc::make_mut(&mut self.checks).push(check);
        self
    }

    pub fn router(self) -> Router {
        Router::new()
            .route("/healthz", get(liveness))
            .route("/readyz", get(readiness))
            .with_state(self)
    }

    pub async fn readiness(&self) -> ReadinessReport {
        let mut ready = true;
        let mut checks = BTreeMap::new();

        for check in self.checks.iter() {
            match check.check().await {
                Ok(()) => {
                    checks.insert(
                        check.name().to_owned(),
                        CheckResult {
                            ok: true,
                            error: None,
                        },
                    );
                }
                Err(error) => {
                    ready = false;
                    checks.insert(
                        check.name().to_owned(),
                        CheckResult {
                            ok: false,
                            error: Some(error),
                        },
                    );
                }
            }
        }

        ReadinessReport { ready, checks }
    }
}

#[derive(Debug, Serialize)]
pub struct ReadinessReport {
    pub ready: bool,
    pub checks: BTreeMap<String, CheckResult>,
}

#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
struct StatusResponse {
    status: &'static str,
}

async fn liveness() -> impl IntoResponse {
    (StatusCode::OK, Json(StatusResponse { status: "ok" }))
}

async fn readiness(State(health): State<Health>) -> impl IntoResponse {
    let report = health.readiness().await;
    let status = if report.ready { "ready" } else { "unready" };
    let code = if report.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        code,
        Json(ReadinessResponse {
            status,
            checks: report.checks,
        }),
    )
}

#[derive(Serialize)]
struct ReadinessResponse {
    status: &'static str,
    checks: BTreeMap<String, CheckResult>,
}

#[cfg(test)]
mod tests {
    use super::{check_fn, Health};

    #[tokio::test]
    async fn readiness_reports_failed_checks() {
        let health = Health::new()
            .with_check(check_fn("ok", || async { Ok(()) }))
            .with_check(check_fn("broken", || async { Err("nope".to_owned()) }));

        let report = health.readiness().await;
        assert!(!report.ready);
        assert!(report.checks["ok"].ok);
        assert_eq!(report.checks["broken"].error.as_deref(), Some("nope"));
    }
}

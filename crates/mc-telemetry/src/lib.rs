pub mod health;

pub use health::{check_fn, CheckResult, Health, ReadinessCheck, ReadinessReport};
use tracing_subscriber::EnvFilter;

#[derive(Clone, Debug)]
pub struct TelemetryConfig {
    pub json: bool,
    pub filter: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            json: true,
            filter: "info".to_owned(),
        }
    }
}

impl TelemetryConfig {
    #[must_use]
    pub fn new(json: bool, filter: impl Into<String>) -> Self {
        Self {
            json,
            filter: filter.into(),
        }
    }
}

pub fn init(config: &TelemetryConfig) {
    let filter = EnvFilter::try_new(&config.filter).unwrap_or_else(|error| {
        eprintln!(
            "invalid log filter {:?}: {error}, falling back to info",
            config.filter
        );
        EnvFilter::new("info")
    });

    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::stderr);

    if config.json {
        builder
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .init();
    } else {
        builder.init();
    }
}

#[cfg(feature = "metrics")]
pub use metrics_exporter_prometheus::PrometheusHandle;

#[cfg(feature = "metrics")]
pub fn install_metrics() -> Result<PrometheusHandle, String> {
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .map_err(|error| error.to_string())
}

#[cfg(feature = "metrics")]
pub fn metrics_router(handle: PrometheusHandle) -> axum::Router {
    use axum::extract::State;
    use axum::routing::get;
    use axum::Router;

    async fn render(State(handle): State<PrometheusHandle>) -> String {
        handle.render()
    }

    Router::new()
        .route("/metrics", get(render))
        .with_state(handle)
}

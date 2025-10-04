//! Telemetry bootstrap for server (tracing + optional Prometheus metrics).

use anyhow::Result;

pub struct TelemetryGuard;

pub fn init_telemetry(cfg: &data_runtime::configs::telemetry::TelemetryCfg) -> Result<TelemetryGuard> {
    use tracing_subscriber::{fmt, EnvFilter, prelude::*};
    let level = cfg.log_level.clone().unwrap_or_else(|| "info".to_string());
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));
    // Console JSON by default
    let fmt_layer = if cfg.json_logs.unwrap_or(true) {
        fmt::layer().json().boxed()
    } else {
        fmt::layer().boxed()
    };
    // Build registry
    let registry = tracing_subscriber::registry().with(filter).with(fmt_layer);
    registry.init();
    // Optional Prometheus metrics exporter
    if let Some(addr) = &cfg.metrics_addr {
        let addr = addr.parse().unwrap_or_else(|_| "127.0.0.1:9100".parse().unwrap());
        let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
        let _ = builder.with_http_listener(addr).install();
    }
    Ok(TelemetryGuard)
}


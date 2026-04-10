use crate::error::MonitorError;
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace::TracerProvider, Resource};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Holds the tracer provider and shuts it down when dropped.
pub struct TelemetryGuard {
    _provider: TracerProvider,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        global::shutdown_tracer_provider();
    }
}

/// Initialise tracing with structured JSON logging and (optionally) OTLP export.
///
/// - `service_name`: reported as the OTel `service.name` resource attribute.
/// - `openobserve_url`: base URL of the OpenObserve instance (e.g. `http://localhost:5080`).
///   Set to `""` to disable OTLP export and log to stdout only.
/// - `openobserve_token`: Bearer token for the OpenObserve API.
/// - `log_level`: `tracing` filter string, e.g. `"info"`.
pub fn init_telemetry(
    service_name: &str,
    openobserve_url: &str,
    openobserve_token: &str,
    log_level: &str,
) -> Result<TelemetryGuard, MonitorError> {
    let filter = EnvFilter::try_new(log_level).unwrap_or_else(|_| EnvFilter::new("info"));
    let json_layer = tracing_subscriber::fmt::layer().json();

    if openobserve_url.is_empty() {
        // Stdout only — no OTLP export
        let provider = TracerProvider::default();
        global::set_tracer_provider(provider.clone());

        tracing_subscriber::registry()
            .with(filter)
            .with(json_layer)
            .try_init()
            .map_err(|e| MonitorError::InitError(e.to_string()))?;

        return Ok(TelemetryGuard {
            _provider: provider,
        });
    }

    // Build the OTLP/HTTP span exporter pointing at OpenObserve
    let endpoint = format!("{}/api/default", openobserve_url.trim_end_matches('/'));

    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        format!("Bearer {}", openobserve_token),
    );

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| MonitorError::OtelError(e.to_string()))?;

    let resource = Resource::new(vec![opentelemetry::KeyValue::new(
        "service.name",
        service_name.to_owned(),
    )]);

    let provider = TracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter, runtime::Tokio)
        .build();

    global::set_tracer_provider(provider.clone());

    // Obtain a typed SDK tracer directly so it satisfies `PreSampledTracer`
    let tracer = opentelemetry::trace::TracerProvider::tracer(
        &provider,
        std::borrow::Cow::Owned(service_name.to_owned()),
    );
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(filter)
        .with(json_layer)
        .with(otel_layer)
        .try_init()
        .map_err(|e| MonitorError::InitError(e.to_string()))?;

    Ok(TelemetryGuard {
        _provider: provider,
    })
}

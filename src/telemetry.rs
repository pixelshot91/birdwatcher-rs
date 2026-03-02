use color_eyre::eyre::{Context, Result};
use opentelemetry::global;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;

fn build_meter_provider() -> Result<SdkMeterProvider, opentelemetry_otlp::ExporterBuildError> {
    // Initialize OTLP exporter using gRPC
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .build()?;

    // Create a meter provider with the OTLP Metric exporter
    let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .build();

    Ok(meter_provider)
}

fn build_logger_provider(
) -> Result<opentelemetry_sdk::logs::SdkLoggerProvider, opentelemetry_otlp::ExporterBuildError> {
    let log_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .build()?;

    let logger_provider = opentelemetry_sdk::logs::SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .build();

    Ok(logger_provider)
}

fn build_tracer_provider(
) -> Result<opentelemetry_sdk::trace::SdkTracerProvider, opentelemetry_otlp::ExporterBuildError> {
    // Initialize OTLP exporter using gRPC
    let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()?;

    // Create a tracer provider with the exporter
    let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(otlp_exporter)
        .build();

    Ok(tracer_provider)
}

fn build_tracing_subscriber(
    logger_provider: &SdkLoggerProvider,
    tracer_provider: &SdkTracerProvider,
) -> impl tracing::Subscriber + std::marker::Send + std::marker::Sync + 'static {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::Layer;
    use tracing_subscriber::Registry;

    let otel_layer: OpenTelemetryTracingBridge<
        opentelemetry_sdk::logs::SdkLoggerProvider,
        opentelemetry_sdk::logs::SdkLogger,
    > = OpenTelemetryTracingBridge::new(logger_provider);

    // Note: the tracer name 'birdwatcher' does not appear to be used
    let tracer = tracer_provider.tracer("birdwatcher");

    // Create a tracing layer with the configured tracer
    let trace_otlp_exporter_layer: tracing_opentelemetry::OpenTelemetryLayer<
        Registry,
        opentelemetry_sdk::trace::Tracer,
    > = tracing_opentelemetry::layer().with_tracer(tracer);

    let trace_otlp_exporter_layer =
        trace_otlp_exporter_layer.with_filter(EnvFilter::from_env("BIRDWATCHER_TRACE_LEVEL"));

    let log_stdout_exporter_layer = tracing_subscriber::fmt::Layer::new()
        .with_filter(EnvFilter::from_env("BIRDWATCHER_LOG_LEVEL"));

    // To prevent a telemetry-induced-telemetry loop
    // See: https://github.com/open-telemetry/opentelemetry-rust/pull/3084/files#diff-b3b130c078a1640592a5defce7c923f8343047e7f18c71e0707bc4a0f094e731L69
    let filter_otel = EnvFilter::from_env("BIRDWATCHER_LOG_LEVEL")
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap());
    let log_otlp_exporter_layer = otel_layer.with_filter(filter_otel);

    Registry::default()
        .with(trace_otlp_exporter_layer)
        .with(log_otlp_exporter_layer)
        .with(log_stdout_exporter_layer)
}

pub fn init_telemetry() -> Result<(SdkMeterProvider, SdkLoggerProvider, SdkTracerProvider)> {
    let meter_provider = build_meter_provider()?;
    global::set_meter_provider(meter_provider.clone());

    let logger_provider = build_logger_provider()?;

    let tracer_provider = build_tracer_provider()?;

    let tracing_subscriber = build_tracing_subscriber(&logger_provider, &tracer_provider);

    tracing::subscriber::set_global_default(tracing_subscriber)
        .context("While setting global tracing subscriber")?;

    Ok((meter_provider, logger_provider, tracer_provider))
}

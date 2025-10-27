use std::time::Duration;

use opentelemetry::global;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::{info, info_span, trace, warn, Level};

fn build_meter_provider() -> Result<SdkMeterProvider, opentelemetry_otlp::ExporterBuildError> {
    // Initialize OTLP exporter using gRPC
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .build()?;

    let b = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
        .with_interval(Duration::from_secs(4));
    // Create a meter provider with the OTLP Metric exporter
    let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        // .with_periodic_exporter(exporter)
        .with_reader(b.build())
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
    stdout_log_level: tracing::metadata::Level,
) -> Result<impl tracing::Subscriber + Send + Sync + 'static, opentelemetry_otlp::ExporterBuildError>
{
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::Layer;
    use tracing_subscriber::Registry;

    let otel_layer: OpenTelemetryTracingBridge<
        opentelemetry_sdk::logs::SdkLoggerProvider,
        opentelemetry_sdk::logs::SdkLogger,
    > = OpenTelemetryTracingBridge::new(&logger_provider);

    let tracer = tracer_provider.tracer("readme_example");

    // Create a tracing layer with the configured tracer
    let trace_otlp_exporter_layer: tracing_opentelemetry::OpenTelemetryLayer<
        Registry,
        opentelemetry_sdk::trace::Tracer,
    > = tracing_opentelemetry::layer().with_tracer(tracer);

    use tracing_subscriber::EnvFilter;
    let log_stdout_exporter_layer = tracing_subscriber::fmt::Layer::new()
        .with_filter(EnvFilter::from_default_env().add_directive(stdout_log_level.into()));

    // To prevent a telemetry-induced-telemetry loop
    // See: https://github.com/open-telemetry/opentelemetry-rust/pull/3084/files#diff-b3b130c078a1640592a5defce7c923f8343047e7f18c71e0707bc4a0f094e731L69
    /* let filter_otel = EnvFilter::new("info")
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap());
    let log_otlp_exporter_layer = otel_layer.with_filter(filter_otel); */

    // Use the tracing subscriber `Registry`, or any other subscriber
    // that impls `LookupSpan`
    let subscriber = Registry::default()
        // .with(trace_otlp_exporter_layer)
        // .with(log_otlp_exporter_layer)
        .with(log_stdout_exporter_layer);

    Ok(subscriber)
}

fn init_telemetry() -> Result<
    (SdkMeterProvider, SdkLoggerProvider, SdkTracerProvider),
    opentelemetry_otlp::ExporterBuildError,
> {
    let meter_provider = build_meter_provider()?;
    global::set_meter_provider(meter_provider.clone());

    let logger_provider = build_logger_provider()?;

    let tracer_provider = build_tracer_provider()?;

    let tracing_subscriber =
        build_tracing_subscriber(&logger_provider, &tracer_provider, Level::WARN)?;

    tracing::subscriber::set_global_default(tracing_subscriber).unwrap();

    Ok((meter_provider, logger_provider, tracer_provider))
}

fn send_dummy_telemetry(
    meter_provider: &SdkMeterProvider,
    logger_provider: &SdkLoggerProvider,
    tracer_provider: &SdkTracerProvider,
) -> OTelSdkResult {
    {
        let _test_span = info_span!("my_test_span").entered();

        trace!("Creating a dummy event");
        info!("This is an info event");
        warn!("This is a warn event");

        let meter = global::meter("my_test_meter");
        let counter = meter.u64_counter("my_test_counter").build();
        counter.add(1, &[]);
    }

    tracer_provider.force_flush().unwrap();
    logger_provider.force_flush().unwrap();
    meter_provider.force_flush().unwrap();
    Ok(())
}

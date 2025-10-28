use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::{Span, SpanContext, TraceFlags, TraceState, Tracer};
use opentelemetry_sdk::trace::{IdGenerator, RandomIdGenerator};
use reqwest::Client;
use tokio::time::sleep;

use o11y::ResourceConfig;
use o11y::logger::{self, LoggerConfig};
use o11y::meter::{self, MeterConfig, RuntimeConfig};
use o11y::tracer::{self, TracerConfig};

#[cfg(all(unix, feature = "profiler"))]
use o11y::profiler::{self, ProfilerConfig};

#[path = "common/mod.rs"]
mod common;

use common::{
    ObservationCase, Targets, emit_log, record_metric, wait_for_loki, wait_for_mimir,
    wait_for_tempo,
};
#[cfg(all(unix, feature = "profiler"))]
use common::{generate_cpu_load, wait_for_pyroscope};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_logger_pushes_to_loki() -> Result<()> {
    let endpoints = Targets::with_env_overrides();
    let case = ObservationCase::new("standalone-logger");

    let resource = ResourceConfig::new(&case.service_name).build();

    let logger_config = LoggerConfig::new(&case.service_name)
        .with_endpoint(endpoints.logs_otel_url.clone());

    let provider = logger::setup(&logger_config, &resource)?
        .ok_or_else(|| anyhow!("logger provider not initialised"))?;

    let (span_context, trace_id, span_id) = random_span_context();

    emit_log(&provider, &span_context, &case.log_message, &case.test_case)?;

    logger::shutdown(provider);

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("build reqwest client")?;

    wait_for_loki(
        &client,
        &endpoints.loki_query_url,
        &case.service_name,
        &case.log_message,
        &case.test_case,
        &trace_id,
        &span_id,
    )
    .await
    .context("loki verification failed")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_tracer_pushes_to_tempo() -> Result<()> {
    let endpoints = Targets::with_env_overrides();
    let case = ObservationCase::new("standalone-tracer");

    let resource = ResourceConfig::new(&case.service_name).build();

    let tracer_config = TracerConfig::new(&case.service_name)
        .with_endpoint(endpoints.traces_otel_endpoint.clone())
        .use_global(true);

    let provider = tracer::init(&tracer_config, &resource)?
        .ok_or_else(|| anyhow!("tracer provider not initialised"))?;

    let tracer = global::tracer("rust-o11y/standalone-tests");
    let mut span = tracer.start("standalone-telemetry-span");
    span.set_attribute(KeyValue::new("test_case", case.test_case.clone()));
    let span_context = span.span_context().clone();
    let trace_id = span_context.trace_id().to_string();

    sleep(Duration::from_secs(2)).await;
    drop(span);

    tracer::shutdown(provider);

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("build reqwest client")?;

    wait_for_tempo(
        &client,
        &endpoints.tempo_query_url,
        &case.service_name,
        &case.test_case,
        &trace_id,
    )
    .await
    .context("tempo verification failed")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_meter_pushes_to_mimir() -> Result<()> {
    let endpoints = Targets::with_env_overrides();
    let case = ObservationCase::new("standalone-meter");

    let resource = ResourceConfig::new(&case.service_name).build();

    let meter_config = MeterConfig::new(&case.service_name)
        .with_endpoint(endpoints.metrics_otel_url.clone())
        .with_export_interval(Duration::from_millis(200))
        .with_runtime(RuntimeConfig::default())
        .use_global(true);

    let provider = meter::init(&meter_config, &resource)?
        .ok_or_else(|| anyhow!("meter provider not initialised"))?;

    let (_, trace_id, span_id) = random_span_context();

    record_metric(&case.metric_name, &case.test_case, &trace_id, &span_id).await?;

    sleep(Duration::from_secs(2)).await;

    meter::shutdown(provider);

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("build reqwest client")?;

    wait_for_mimir(
        &client,
        &endpoints.mimir_query_url,
        &case.metric_name,
        &case.test_case,
        &trace_id,
        &span_id,
    )
    .await
    .context("mimir verification failed")?;

    Ok(())
}

#[cfg(all(unix, feature = "profiler"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_profiler_pushes_to_pyroscope() -> Result<()> {
    let endpoints = Targets::with_env_overrides();
    let case = ObservationCase::new("standalone-profiler");

    let profiler_config = ProfilerConfig::new(&case.service_name)
        .with_server_url(endpoints.pyroscope_url.clone())
        .with_tag("test_case", case.test_case.clone())
        .with_tenant_id(endpoints.pyroscope_tenant.clone());

    let agent = profiler::setup(&profiler_config)?
        .ok_or_else(|| anyhow!("profiler agent not initialised"))?;

    generate_cpu_load(Duration::from_secs(2)).await;
    sleep(Duration::from_secs(2)).await;

    profiler::shutdown(agent);

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("build reqwest client")?;

    wait_for_pyroscope(
        &client,
        &endpoints.pyroscope_url,
        &endpoints.pyroscope_tenant,
        &case.service_name,
        &case.test_case,
    )
    .await
    .context("pyroscope verification failed")?;

    Ok(())
}

fn random_span_context() -> (SpanContext, String, String) {
    let generator = RandomIdGenerator::default();
    let trace_id = generator.new_trace_id();
    let span_id = generator.new_span_id();
    let context = SpanContext::new(
        trace_id,
        span_id,
        TraceFlags::SAMPLED,
        true,
        TraceState::default(),
    );

    (context, trace_id.to_string(), span_id.to_string())
}

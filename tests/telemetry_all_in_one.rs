use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::{Span, Tracer};
use reqwest::Client;
use tokio::time::sleep;

use o11y::logger::LoggerConfig;
use o11y::meter::{MeterConfig, RuntimeConfig};
use o11y::profiler::ProfilerConfig;
use o11y::tracer::TracerConfig;
use o11y::{Config, Telemetry};

#[path = "common/mod.rs"]
mod common;

use common::{
    ObservationCase, Targets, emit_log, record_metric, wait_for_loki, wait_for_mimir,
    wait_for_tempo,
};
#[cfg(all(unix, feature = "profiler"))]
use common::{generate_cpu_load, wait_for_pyroscope};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn telemetry_pushes_to_backends() -> Result<()> {
    let endpoints = Targets::with_env_overrides();
    let case = ObservationCase::new("telemetry");

    let mut config = Config::new(&case.service_name)
        .with_logger(
            LoggerConfig::new(&case.service_name).with_endpoint(endpoints.logs_otel_url.clone()),
        )
        .with_tracer(
            TracerConfig::new(&case.service_name)
                .with_endpoint(endpoints.traces_otel_endpoint.clone())
                .use_global(true),
        )
        .with_meter(
            MeterConfig::new(&case.service_name)
                .with_endpoint(endpoints.metrics_otel_url.clone())
                .with_export_interval(Duration::from_millis(200))
                .with_runtime(RuntimeConfig::default())
                .use_global(true),
        );

    #[cfg(all(unix, feature = "profiler"))]
    {
        config = config.with_profiler(
            ProfilerConfig::new(&case.service_name)
                .with_server_url(endpoints.pyroscope_url.clone())
                .with_tag("test_case", case.test_case.clone())
                .with_tenant_id(endpoints.pyroscope_tenant.clone()),
        );
    }

    let telemetry = Telemetry::new(config).context("setup telemetry")?;

    #[cfg(all(unix, feature = "profiler"))]
    let expect_profiler = telemetry.profiler.is_some();
    #[cfg(not(all(unix, feature = "profiler")))]
    let expect_profiler = false;

    let tracer = global::tracer("rust-o11y/tests");
    let mut span = tracer.start("telemetry-integration-span");
    span.set_attribute(KeyValue::new("test_case", case.test_case.clone()));
    let span_context = span.span_context().clone();
    let trace_id_str = span_context.trace_id().to_string();
    let span_id_str = span_context.span_id().to_string();

    let logger_provider = telemetry
        .logger
        .as_ref()
        .ok_or_else(|| anyhow!("logger provider not initialised"))?;

    emit_log(
        logger_provider,
        &span_context,
        &case.log_message,
        &case.test_case,
    )?;
    record_metric(
        &case.metric_name,
        &case.test_case,
        &trace_id_str,
        &span_id_str,
    )
    .await?;

    #[cfg(all(unix, feature = "profiler"))]
    if expect_profiler {
        generate_cpu_load(Duration::from_secs(2)).await;
    }

    sleep(Duration::from_secs(2)).await;
    span.end();
    telemetry.shutdown();

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
        &trace_id_str,
        &span_id_str,
    )
    .await
    .context("loki verification failed")?;

    wait_for_mimir(
        &client,
        &endpoints.mimir_query_url,
        &case.metric_name,
        &case.test_case,
        &trace_id_str,
        &span_id_str,
    )
    .await
    .context("mimir verification failed")?;

    wait_for_tempo(
        &client,
        &endpoints.tempo_query_url,
        &case.service_name,
        &case.test_case,
        &trace_id_str,
    )
    .await
    .context("tempo verification failed")?;

    #[cfg(all(unix, feature = "profiler"))]
    if expect_profiler {
        wait_for_pyroscope(
            &client,
            &endpoints.pyroscope_url,
            &endpoints.pyroscope_tenant,
            &case.service_name,
            &case.test_case,
        )
        .await
        .context("pyroscope verification failed")?;
    }

    Ok(())
}

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use opentelemetry::global;
use opentelemetry::trace::{Span, Tracer};
use opentelemetry::KeyValue;
use reqwest::Client;
use tokio::time::sleep;

use o11y::logger::{self, LoggerConfig};
use o11y::meter::{self, MeterConfig, RuntimeConfig};
use o11y::profiler::ProfilerConfig;
use o11y::tracer::{self, TracerConfig};
use o11y::Config;

#[path = "common/mod.rs"]
mod common;

use common::{emit_log, record_metric, wait_for_loki, wait_for_mimir, wait_for_tempo, ObservationCase, Targets};
#[cfg(all(unix, feature = "profiler"))]
use common::{generate_cpu_load, wait_for_pyroscope};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_components_push_to_backends() -> Result<()> {
    let endpoints = Targets::with_env_overrides();
    let case = ObservationCase::new("standalone");

    let mut base_config = Config::new(&case.service_name);
    base_config.apply_defaults();
    let resource = base_config.resource.build();

    let logger_config = LoggerConfig::new(&case.service_name)
        .enabled(true)
        .with_endpoint(endpoints.logs_otel_url.clone());

    let logger_provider = logger::setup(&logger_config, &resource)?
        .ok_or_else(|| anyhow!("logger provider not initialised"))?;

    let tracer_config = TracerConfig::new(&case.service_name)
        .enabled(true)
        .with_endpoint(endpoints.traces_otel_endpoint.clone())
        .use_global(true);

    let tracer_provider = tracer::init(&tracer_config, &resource)?
        .ok_or_else(|| anyhow!("tracer provider not initialised"))?;

    let meter_config = MeterConfig::new(&case.service_name)
        .enabled(true)
        .with_endpoint(endpoints.metrics_otel_url.clone())
        .with_export_interval(Duration::from_millis(200))
        .with_runtime(RuntimeConfig::default().enabled(false))
        .use_global(true);

    let meter_provider = meter::init(&meter_config, &resource)?
        .ok_or_else(|| anyhow!("meter provider not initialised"))?;

    #[cfg(all(unix, feature = "profiler"))]
    let profiler_agent = match o11y::profiler::setup(
        &ProfilerConfig::new(&case.service_name)
            .enabled(true)
            .with_server_url(endpoints.pyroscope_url.clone())
            .with_tag("test_case", case.test_case.clone())
            .with_tenant_id(endpoints.pyroscope_tenant.clone()),
    ) {
        Ok(agent) => agent,
        Err(err) => {
            eprintln!("skipping profiler collection: {err}");
            None
        }
    };

    #[cfg(all(unix, feature = "profiler"))]
    let expect_profiler = profiler_agent.is_some();
    #[cfg(not(all(unix, feature = "profiler")))]
    let expect_profiler = false;

    let tracer = global::tracer("rust-o11y/standalone-tests");
    let mut span = tracer.start("standalone-telemetry-span");
    span.set_attribute(KeyValue::new("test_case", case.test_case.clone()));
    let span_context = span.span_context().clone();
    let trace_id_str = span_context.trace_id().to_string();
    let span_id_str = span_context.span_id().to_string();

    emit_log(&logger_provider, &span_context, &case.log_message, &case.test_case)?;
    record_metric(&case.metric_name, &case.test_case, &trace_id_str, &span_id_str).await?;

    #[cfg(all(unix, feature = "profiler"))]
    if expect_profiler {
        generate_cpu_load(Duration::from_secs(2)).await;
    }

    sleep(Duration::from_secs(2)).await;
    span.end();

    logger::shutdown(logger_provider);
    tracer::shutdown(tracer_provider);
    meter::shutdown(meter_provider);

    #[cfg(all(unix, feature = "profiler"))]
    if let Some(agent) = profiler_agent {
        o11y::profiler::shutdown(agent);
    }

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

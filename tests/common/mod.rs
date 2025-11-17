use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use opentelemetry::KeyValue;
use opentelemetry::logs::{AnyValue, LogRecord as _, Logger as _, LoggerProvider as _, Severity};
use opentelemetry::trace::SpanContext;
use opentelemetry_sdk::logs::LogRecord;
use reqwest::{Client, StatusCode};
use serde_json::Value;
use tokio::time::{Instant, sleep};

use o11y::logger::LoggerProvider;

pub struct ObservationCase {
    pub service_name: String,
    pub test_case: String,
    pub log_message: String,
    pub metric_name: String,
}

impl ObservationCase {
    pub fn new(prefix: &str) -> Self {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();

        Self {
            service_name: format!("{}-service-{}", prefix, now_ns),
            test_case: format!("{}-test-{}", prefix, now_ns),
            log_message: format!("{}-log-{}", prefix, now_ns),
            metric_name: format!("{}_metric_total_{}", prefix.replace('-', "_"), now_ns),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Targets {
    pub logs_otel_url: String,
    pub loki_query_url: String,
    pub metrics_otel_url: String,
    pub mimir_query_url: String,
    pub traces_otel_endpoint: String,
    pub tempo_query_url: String,
    pub pyroscope_url: String,
    pub pyroscope_tenant: String,
}

impl Targets {
    pub fn default() -> Self {
        Self {
            logs_otel_url: "http://localhost:3100/otlp".to_string(),
            loki_query_url: "http://localhost:3100".to_string(),
            metrics_otel_url: "http://localhost:4318".to_string(),
            mimir_query_url: "http://localhost:9009".to_string(),
            traces_otel_endpoint: "http://localhost:4317".to_string(),
            tempo_query_url: "http://localhost:3200".to_string(),
            pyroscope_url: "http://localhost:4040".to_string(),
            pyroscope_tenant: "anonymous".to_string(),
        }
    }

    pub fn with_env_overrides() -> Self {
        let mut targets = Self::default();

        if let Ok(value) = std::env::var("LOGS_OTEL_URL") {
            targets.logs_otel_url = value;
        }
        if let Ok(value) = std::env::var("LOKI_QUERY_URL") {
            targets.loki_query_url = value;
        }
        if let Ok(value) = std::env::var("METRICS_OTEL_URL") {
            targets.metrics_otel_url = value;
        }
        if let Ok(value) = std::env::var("MIMIR_QUERY_URL") {
            targets.mimir_query_url = value;
        }
        if let Ok(value) = std::env::var("TRACES_OTEL_ENDPOINT") {
            targets.traces_otel_endpoint = value;
        }
        if let Ok(value) = std::env::var("TEMPO_QUERY_URL") {
            targets.tempo_query_url = value;
        }
        if let Ok(value) = std::env::var("PYROSCOPE_URL") {
            targets.pyroscope_url = value;
        }
        if let Ok(value) = std::env::var("PYROSCOPE_TENANT") {
            targets.pyroscope_tenant = value;
        }

        targets
    }
}

pub fn emit_log(
    provider: &LoggerProvider,
    span_context: &SpanContext,
    message: &str,
    test_case: &str,
) -> Result<()> {
    let logger = provider.logger("rust-o11y/tests");

    let mut record = LogRecord::default();
    record.set_body(AnyValue::from(message.to_string()));
    record.set_timestamp(SystemTime::now());
    record.set_observed_timestamp(SystemTime::now());
    record.set_severity_number(Severity::Info);
    record.set_severity_text("INFO");
    record.add_attribute("message", message.to_string());
    record.add_attribute("test_case", test_case.to_string());
    record.add_attribute("trace_id", span_context.trace_id().to_string());
    record.add_attribute("span_id", span_context.span_id().to_string());
    record.set_trace_context(
        span_context.trace_id(),
        span_context.span_id(),
        Some(span_context.trace_flags()),
    );

    logger.emit(record);

    for result in provider.force_flush() {
        result?;
    }

    Ok(())
}

pub async fn record_metric(
    metric_name: &str,
    test_case: &str,
    trace_id: &str,
    span_id: &str,
) -> Result<()> {
    let meter = opentelemetry::global::meter("rust-o11y/tests");
    let metric_name_static: &'static str = Box::leak(metric_name.to_string().into_boxed_str());
    let counter = meter
        .u64_counter(metric_name_static)
        .with_description("integration test counter")
        .build();

    let attrs = [
        KeyValue::new("test_case", test_case.to_string()),
        KeyValue::new("trace_id", trace_id.to_string()),
        KeyValue::new("span_id", span_id.to_string()),
    ];

    counter.add(1, &attrs);

    Ok(())
}

pub async fn wait_for_loki(
    client: &Client,
    base: &str,
    service_name: &str,
    message: &str,
    test_case: &str,
    trace_id: &str,
    span_id: &str,
) -> Result<()> {
    let base = normalize_loki_base(base);
    let mut url = reqwest::Url::parse(&format!("{}/loki/api/v1/query_range", base))?;

    let now_ns = now_nanos()?;
    url.query_pairs_mut()
        .append_pair("start", &(now_ns - 60_000_000_000).to_string())
        .append_pair("end", &(now_ns + 60_000_000_000).to_string())
        .append_pair("limit", "200")
        .append_pair("direction", "BACKWARD")
        .append_pair("query", &format!("{{service_name=\"{}\"}}", service_name));

    wait_until(Duration::from_secs(45), Duration::from_millis(500), || {
        let client = client.clone();
        let url = url.clone();
        let message = message.to_string();
        let test_case = test_case.to_string();
        let trace_id = trace_id.to_string();
        let span_id = span_id.to_string();
        async move {
            let resp = client.get(url.clone()).send().await?;
            if resp.status() == StatusCode::NOT_FOUND {
                return Ok(false);
            }
            if !resp.status().is_success() {
                return Ok(false);
            }
            let body: Value = resp.json().await?;
            if let Some(results) = body["data"]["result"].as_array() {
                for result in results {
                    let stream = result["stream"].as_object();
                    let trace_ok = stream
                        .and_then(|s| s.get("trace_id"))
                        .and_then(|v| v.as_str())
                        .map(|s| s == trace_id)
                        .unwrap_or(false);
                    let span_ok = stream
                        .and_then(|s| s.get("span_id"))
                        .and_then(|v| v.as_str())
                        .map(|s| s == span_id)
                        .unwrap_or(false);
                    let case_ok = stream
                        .and_then(|s| s.get("test_case"))
                        .and_then(|v| v.as_str())
                        .map(|s| s == test_case)
                        .unwrap_or(false);

                    if !trace_ok || !span_ok || !case_ok {
                        continue;
                    }

                    if let Some(values) = result["values"].as_array() {
                        for tuple in values {
                            if let Some(line) = tuple.get(1).and_then(|v| v.as_str())
                                && (line.contains(&message)
                                    || matches_loki_line(line, &message, &trace_id, &span_id))
                            {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
            Ok(false)
        }
    })
    .await
}

pub async fn wait_for_mimir(
    client: &Client,
    base: &str,
    metric_name: &str,
    test_case: &str,
    trace_id: &str,
    span_id: &str,
) -> Result<()> {
    let mut url = reqwest::Url::parse(&format!(
        "{}/prometheus/api/v1/query",
        base.trim_end_matches('/')
    ))?;
    let query = format!(
        "{}{{test_case=\"{}\",trace_id=\"{}\",span_id=\"{}\"}}",
        metric_name, test_case, trace_id, span_id
    );
    url.query_pairs_mut().append_pair("query", &query);

    wait_until(Duration::from_secs(45), Duration::from_millis(500), || {
        let client = client.clone();
        let url = url.clone();
        async move {
            let resp = client.get(url.clone()).send().await?;
            if !resp.status().is_success() {
                return Ok(false);
            }
            let body: Value = resp.json().await?;
            if body["status"].as_str() != Some("success") {
                return Ok(false);
            }
            if let Some(results) = body["data"]["result"].as_array() {
                if results.is_empty() {
                    return Ok(false);
                }
                if let Some(value) = results[0]["value"].as_array()
                    && value.len() == 2
                    && let Some(sample) = value[1].as_str()
                    && sample != "0"
                {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    })
    .await
}

pub async fn wait_for_tempo(
    client: &Client,
    base: &str,
    service_name: &str,
    test_case: &str,
    trace_id: &str,
) -> Result<()> {
    let mut url = reqwest::Url::parse(&format!("{}/api/search", base.trim_end_matches('/')))?;
    url.query_pairs_mut()
        .append_pair("limit", "5")
        .append_pair("tags", &format!("service.name={}", service_name))
        .append_pair("tags", &format!("test_case={}", test_case));

    wait_until(Duration::from_secs(45), Duration::from_millis(500), || {
        let client = client.clone();
        let url = url.clone();
        let trace_id = trace_id.to_string();
        async move {
            let resp = client.get(url.clone()).send().await?;
            if resp.status() == StatusCode::NOT_FOUND {
                return Ok(false);
            }
            if !resp.status().is_success() {
                return Ok(false);
            }
            let body: Value = resp.json().await?;
            if let Some(traces) = body["traces"].as_array() {
                for trace in traces {
                    if trace["traceID"].as_str() == Some(&trace_id) {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        }
    })
    .await
}

#[cfg(all(unix, feature = "profiler"))]
pub async fn wait_for_pyroscope(
    client: &Client,
    base: &str,
    tenant_id: &str,
    service_name: &str,
    test_case: &str,
) -> Result<()> {
    let mut url = reqwest::Url::parse(&format!("{}/pyroscope/render", base.trim_end_matches('/')))?;
    let query = format!(
        "process_cpu:cpu:nanoseconds:cpu:nanoseconds{{service=\"{}\",test_case=\"{}\"}}",
        service_name, test_case
    );
    url.query_pairs_mut()
        .append_pair("query", &query)
        .append_pair("from", "now-5m")
        .append_pair("until", "now");

    wait_until(Duration::from_secs(60), Duration::from_millis(500), || {
        let client = client.clone();
        let url = url.clone();
        let tenant = tenant_id.to_string();
        async move {
            let mut request = client.get(url.clone());
            if !tenant.is_empty() {
                request = request.header("X-Scope-OrgID", tenant.clone());
            }

            let resp = request.send().await?;
            if resp.status() == StatusCode::NOT_FOUND {
                return Ok(false);
            }
            if !resp.status().is_success() {
                return Ok(false);
            }

            let body: Value = resp.json().await?;
            let ticks = body
                .get("flamebearer")
                .and_then(|fb| fb.get("numTicks"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            if ticks <= 0 {
                return Ok(false);
            }

            if let Some(metadata) = body.get("metadata")
                && let Some(app_name) = metadata.get("appName").and_then(|v| v.as_str())
                && !app_name.contains(service_name)
            {
                return Ok(false);
            }

            Ok(true)
        }
    })
    .await
}

#[cfg(all(unix, feature = "profiler"))]
pub async fn generate_cpu_load(duration: Duration) {
    use std::time::Instant as StdInstant;

    let _ = tokio::task::spawn_blocking(move || {
        let deadline = StdInstant::now() + duration;
        let mut acc: u64 = 0;
        while StdInstant::now() < deadline {
            for i in 0..50_000 {
                acc = acc.wrapping_mul(31).wrapping_add(i);
            }
            std::hint::black_box(acc);
        }
    })
    .await;
}

pub async fn wait_until<F, Fut>(timeout: Duration, interval: Duration, mut op: F) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<bool>>,
{
    let deadline = Instant::now() + timeout;
    loop {
        if op().await? {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(anyhow!("timed out waiting for condition"));
        }

        sleep(interval).await;
    }
}

fn matches_loki_line(line: &str, message: &str, trace_id: &str, span_id: &str) -> bool {
    if let Ok(json_line) = serde_json::from_str::<Value>(line) {
        let message_match = json_line["message"]
            .as_str()
            .map(|s| s.contains(message))
            .unwrap_or(false);
        let trace_match = json_line["trace_id"]
            .as_str()
            .map(|s| s == trace_id)
            .unwrap_or(false);
        let span_match = json_line["span_id"]
            .as_str()
            .map(|s| s == span_id)
            .unwrap_or(false);
        return message_match && trace_match && span_match;
    }

    line.contains(message) && line.contains(trace_id) && line.contains(span_id)
}

fn normalize_loki_base(raw: &str) -> String {
    let mut trimmed = raw.trim_end_matches('/').to_string();
    if let Some(stripped) = trimmed.strip_suffix("/otlp/v1/logs") {
        trimmed = stripped.to_string();
    }
    if let Some(stripped) = trimmed.strip_suffix("/loki/api/v1/push") {
        trimmed = stripped.to_string();
    }
    trimmed.trim_end_matches('/').to_string()
}

fn now_nanos() -> Result<i128> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(now.as_nanos() as i128)
}

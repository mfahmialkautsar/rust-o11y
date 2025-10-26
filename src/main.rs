mod logger;
mod meter;
mod profiler;
mod tracer;

use anyhow::Result;
use opentelemetry::{
    KeyValue,
    trace::{TraceContextExt, TracerProvider},
};
use opentelemetry_sdk::resource::Resource;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;

const SERVICE_NAME: &str = "rust-clean-arch";
const SERVICE_NAMESPACE: &str = "rust-clean-arch";
const LOG_FILTER_DEFAULT: &str =
    "info,rust_clean_arch=debug,otel::tracing=trace";

pub struct Telemetry {
    tracer_provider: Option<tracer::TracerProvider>,
    meter_provider: Option<meter::MeterProvider>,
    logger_provider: Option<logger::LoggerProvider>,
    pyroscope_agent: Option<profiler::PyroscopeAgent>,
}

impl Telemetry {
    pub fn shutdown(self) {
        if let Some(provider) = self.logger_provider {
            logger::shutdown(provider);
        }
        if let Some(provider) = self.meter_provider {
            meter::shutdown(provider);
        }
        if let Some(agent) = self.pyroscope_agent {
            profiler::shutdown(agent);
        }
        if let Some(provider) = self.tracer_provider {
            tracer::shutdown(provider);
        }
    }

    pub fn has_tracing(&self) -> bool {
        self.tracer_provider.is_some()
    }
}

pub fn init(config: &AppConfig) -> Result<Telemetry> {
    let resource = Resource::builder()
        .with_service_name(config.service_name.clone())
        .with_attributes([KeyValue::new("service.namespace", SERVICE_NAMESPACE)])
        .build();

    let tracer_provider = tracer::init(config, &resource)?;
    let meter_provider = meter::init(config, &resource)?;
    let logger_provider = logger::init(config, &resource)?;

    let build_subscriber = || {
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(LOG_FILTER_DEFAULT));
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_line_number(true)
            .with_file(true)
            .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339());
        Registry::default().with(env_filter).with(fmt_layer)
    };

    match (tracer_provider.as_ref(), logger_provider.as_ref()) {
        (Some(tp), Some(lp)) => {
            let tracer = tp.tracer(SERVICE_NAME);
            let log_layer =
                opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(lp);
            build_subscriber()
                .with(tracing_opentelemetry::layer().with_tracer(tracer))
                .with(log_layer)
                .init();
        }
        (Some(tp), None) => {
            let tracer = tp.tracer(SERVICE_NAME);
            build_subscriber()
                .with(tracing_opentelemetry::layer().with_tracer(tracer))
                .init();
        }
        (None, Some(lp)) => {
            let log_layer =
                opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(lp);
            build_subscriber().with(log_layer).init();
        }
        (None, None) => build_subscriber().init(),
    }

    let pyroscope_agent = profiler::init(config)?;

    Ok(Telemetry {
        tracer_provider,
        meter_provider,
        logger_provider,
        pyroscope_agent,
    })
}

pub struct TraceContextInfo {
    pub trace_id: String,
    pub span_id: String,
    pub sampled: bool,
}

impl TraceContextInfo {
    pub fn into_attributes(self) -> [KeyValue; 3] {
        [
            KeyValue::new("trace_id", self.trace_id),
            KeyValue::new("span_id", self.span_id),
            KeyValue::new("trace_sampled", if self.sampled { "true" } else { "false" }),
        ]
    }
}

pub fn current_trace_context() -> Option<TraceContextInfo> {
    let span = tracing::Span::current();
    let context = span.context();
    let span_ref = context.span();
    let span_context = span_ref.span_context();

    if !span_context.is_valid() {
        return None;
    }

    Some(TraceContextInfo {
        trace_id: span_context.trace_id().to_string(),
        span_id: span_context.span_id().to_string(),
        sampled: span_context.trace_flags().is_sampled(),
    })
}

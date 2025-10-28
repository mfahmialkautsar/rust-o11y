use anyhow::Result;
use opentelemetry::KeyValue;
use opentelemetry::trace::{TraceContextExt, TracerProvider as _};
use opentelemetry_sdk::resource::Resource;
use tracing::{Dispatch, Span, dispatcher};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};

use crate::config::Config;
use crate::logger::{self, LoggerProvider};
use crate::meter::{self, MeterProvider};
use crate::profiler::{self, PyroscopeAgent};
use crate::tracer::{self, TracerProvider};

const DEFAULT_LOG_FILTER_SUFFIX: &str = "otel::tracing=trace,axum_tracing_opentelemetry=trace";

pub struct Telemetry {
    pub logger: Option<LoggerProvider>,
    pub tracer: Option<TracerProvider>,
    pub meter: Option<MeterProvider>,
    pub profiler: Option<PyroscopeAgent>,
}

impl Telemetry {
    pub fn new(mut config: Config) -> Result<Self> {
        config.apply_defaults();
        config.validate()?;

        let resource = config.resource.build();

        let logger = setup_logger(&config, &resource)?;
        let tracer = setup_tracer(&config, &resource)?;
        let meter = setup_meter(&config, &resource)?;
        let profiler = setup_profiler(&config)?;

        if let Err(err) = install_tracing_subscriber(&config, tracer.as_ref(), logger.as_ref()) {
            eprintln!("failed to install tracing subscriber: {err}");
        }

        Ok(Self {
            logger,
            tracer,
            meter,
            profiler,
        })
    }

    pub fn shutdown(self) {
        if let Some(provider) = self.logger {
            logger::shutdown(provider);
        }
        if let Some(provider) = self.tracer {
            tracer::shutdown(provider);
        }
        if let Some(provider) = self.meter {
            meter::shutdown(provider);
        }
        if let Some(agent) = self.profiler {
            profiler::shutdown(agent);
        }
    }

    pub fn has_logger(&self) -> bool {
        self.logger.is_some()
    }

    pub fn has_tracer(&self) -> bool {
        self.tracer.is_some()
    }

    pub fn has_meter(&self) -> bool {
        self.meter.is_some()
    }

    pub fn has_profiler(&self) -> bool {
        self.profiler.is_some()
    }
}

fn setup_logger(config: &Config, resource: &Resource) -> Result<Option<LoggerProvider>> {
    logger::setup(&config.logger, resource)
}

fn setup_tracer(config: &Config, resource: &Resource) -> Result<Option<TracerProvider>> {
    if config.tracer.use_global {
        tracer::init(&config.tracer, resource)
    } else {
        tracer::setup(&config.tracer, resource)
    }
}

fn setup_meter(config: &Config, resource: &Resource) -> Result<Option<MeterProvider>> {
    let provider = if config.meter.use_global {
        meter::init(&config.meter, resource)?
    } else {
        meter::setup(&config.meter, resource)?
    };

    if config.meter.runtime.enabled && config.meter.use_global && provider.is_some() {
        let meter_name = config.meter.service_name.clone();
        if let Err(err) = meter::register_runtime_metrics(meter_name) {
            eprintln!("failed to register runtime metrics: {err}");
        }
    }

    Ok(provider)
}

fn setup_profiler(config: &Config) -> Result<Option<PyroscopeAgent>> {
    profiler::setup(&config.profiler)
}

fn install_tracing_subscriber(
    config: &Config,
    tracer: Option<&TracerProvider>,
    logger: Option<&LoggerProvider>,
) -> Result<(), tracing::subscriber::SetGlobalDefaultError> {
    if dispatcher::has_been_set() {
        return Ok(());
    }

    let fallback = fallback_log_filter(&config.resource.service_name);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(fallback));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_line_number(true)
        .with_file(true)
        .with_timer(tracing_subscriber::fmt::time::SystemTime);

    let base = Registry::default().with(env_filter).with(fmt_layer);

    let dispatch = match (tracer, logger) {
        (Some(tp), Some(lp)) => {
            let tracer = tp.tracer(config.tracer.service_name.clone());
            let log_layer =
                opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(lp);
            Dispatch::new(
                base.with(tracing_opentelemetry::layer().with_tracer(tracer))
                    .with(log_layer),
            )
        }
        (Some(tp), None) => {
            let tracer = tp.tracer(config.tracer.service_name.clone());
            Dispatch::new(base.with(tracing_opentelemetry::layer().with_tracer(tracer)))
        }
        (None, Some(lp)) => {
            let log_layer =
                opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(lp);
            Dispatch::new(base.with(log_layer))
        }
        (None, None) => Dispatch::new(base),
    };

    dispatcher::set_global_default(dispatch)
}

fn fallback_log_filter(service_name: &str) -> String {
    format!("info,{service_name}=debug,{DEFAULT_LOG_FILTER_SUFFIX}")
}

#[derive(Debug, Clone)]
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
    let span = Span::current();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        logger::LoggerConfig, meter::MeterConfig, profiler::ProfilerConfig, tracer::TracerConfig,
    };

    #[test]
    fn test_telemetry_all_disabled() {
        let service = "test-service";
        let config = Config::new(service)
            .with_logger(LoggerConfig::new(service).enabled(false))
            .with_tracer(TracerConfig::new(service).enabled(false))
            .with_meter(MeterConfig::new(service).enabled(false))
            .with_profiler(ProfilerConfig::new(service).enabled(false));
        let tele = Telemetry::new(config).unwrap();

        assert!(!tele.has_logger());
        assert!(!tele.has_tracer());
        assert!(!tele.has_meter());
        assert!(!tele.has_profiler());
    }

    #[test]
    fn test_telemetry_validation() {
        let config = Config::new("");
        let result = Telemetry::new(config);
        assert!(result.is_err());
    }
}

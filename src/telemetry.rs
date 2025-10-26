use anyhow::Result;
use opentelemetry_sdk::resource::Resource;

use crate::config::Config;
use crate::logger::{self, LoggerProvider};
use crate::meter::{self, MeterProvider};
use crate::profiler::{self, PyroscopeAgent};
use crate::tracer::{self, TracerProvider};

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
    let provider = meter::setup(&config.meter, resource)?;

    if let Some(ref p) = provider {
        opentelemetry::global::set_meter_provider(p.clone());
    }

    if config.meter.runtime.enabled
        && provider.is_some() {
            let meter_name = config.meter.service_name.clone();
            if let Err(e) = meter::register_runtime_metrics(meter_name) {
                eprintln!("failed to register runtime metrics: {e}");
            }
        }

    Ok(provider)
}

fn setup_profiler(config: &Config) -> Result<Option<PyroscopeAgent>> {
    profiler::setup(&config.profiler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_all_disabled() {
        let config = Config::new("test-service");
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

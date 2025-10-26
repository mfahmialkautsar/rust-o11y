use opentelemetry_sdk::resource::{Resource, ResourceDetector};
use opentelemetry::KeyValue;
use opentelemetry_semantic_conventions::resource;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use crate::logger::LoggerConfig;
use crate::tracer::TracerConfig;
use crate::meter::MeterConfig;
use crate::profiler::ProfilerConfig;

const DEFAULT_SERVICE_VERSION: &str = "0.1.0";
const DEFAULT_SERVICE_NAMESPACE: &str = "default";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("resource.service_name is required")]
    ServiceNameRequired,
    #[error("logger configuration error: {0}")]
    Logger(String),
    #[error("tracer configuration error: {0}")]
    Tracer(String),
    #[error("meter configuration error: {0}")]
    Meter(String),
    #[error("profiler configuration error: {0}")]
    Profiler(String),
}

#[derive(Clone, Debug)]
pub struct Config {
    pub resource: ResourceConfig,
    pub logger: LoggerConfig,
    pub tracer: TracerConfig,
    pub meter: MeterConfig,
    pub profiler: ProfilerConfig,
    pub customizers: Vec<Arc<dyn ResourceCustomizer>>,
}

impl Config {
    pub fn new(service_name: impl Into<String>) -> Self {
        let service_name = service_name.into();
        Self {
            resource: ResourceConfig::new(service_name.clone()),
            logger: LoggerConfig::new(service_name.clone()),
            tracer: TracerConfig::new(service_name.clone()),
            meter: MeterConfig::new(service_name.clone()),
            profiler: ProfilerConfig::new(service_name),
            customizers: Vec::new(),
        }
    }

    pub fn with_resource(mut self, resource: ResourceConfig) -> Self {
        self.resource = resource;
        self
    }

    pub fn with_logger(mut self, logger: LoggerConfig) -> Self {
        self.logger = logger;
        self
    }

    pub fn with_tracer(mut self, tracer: TracerConfig) -> Self {
        self.tracer = tracer;
        self
    }

    pub fn with_meter(mut self, meter: MeterConfig) -> Self {
        self.meter = meter;
        self
    }

    pub fn with_profiler(mut self, profiler: ProfilerConfig) -> Self {
        self.profiler = profiler;
        self
    }

    pub fn apply_defaults(&mut self) {
        if self.resource.service_version.is_empty() {
            self.resource.service_version = DEFAULT_SERVICE_VERSION.to_string();
        }
        if self.resource.service_namespace.is_empty() {
            self.resource.service_namespace = DEFAULT_SERVICE_NAMESPACE.to_string();
        }

        if self.logger.service_name.is_empty() {
            self.logger.service_name = self.resource.service_name.clone();
        }
        if self.tracer.service_name.is_empty() {
            self.tracer.service_name = self.resource.service_name.clone();
        }
        if self.meter.service_name.is_empty() {
            self.meter.service_name = self.resource.service_name.clone();
        }
        if self.profiler.service_name.is_empty() {
            self.profiler.service_name = self.resource.service_name.clone();
        }

        self.logger.apply_defaults();
        self.tracer.apply_defaults();
        self.meter.apply_defaults();
        self.profiler.apply_defaults();
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.resource.service_name.is_empty() {
            return Err(ConfigError::ServiceNameRequired);
        }

        self.logger
            .validate()
            .map_err(|e| ConfigError::Logger(e.to_string()))?;
        self.tracer
            .validate()
            .map_err(|e| ConfigError::Tracer(e.to_string()))?;
        self.meter
            .validate()
            .map_err(|e| ConfigError::Meter(e.to_string()))?;
        self.profiler
            .validate()
            .map_err(|e| ConfigError::Profiler(e.to_string()))?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct ResourceConfig {
    pub service_name: String,
    pub service_version: String,
    pub service_namespace: String,
    pub environment: String,
    pub attributes: HashMap<String, String>,
    pub detectors: Vec<Arc<dyn ResourceDetector>>,
    pub override_factory: Option<Arc<ResourceFactory>>,
}

impl std::fmt::Debug for ResourceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceConfig")
            .field("service_name", &self.service_name)
            .field("service_version", &self.service_version)
            .field("service_namespace", &self.service_namespace)
            .field("environment", &self.environment)
            .field("attributes", &self.attributes)
            .finish()
    }
}

impl ResourceConfig {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            service_version: DEFAULT_SERVICE_VERSION.to_string(),
            service_namespace: DEFAULT_SERVICE_NAMESPACE.to_string(),
            environment: String::new(),
            attributes: HashMap::new(),
            detectors: Vec::new(),
            override_factory: None,
        }
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.service_version = version.into();
        self
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.service_namespace = namespace.into();
        self
    }

    pub fn with_environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = environment.into();
        self
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    pub fn build(&self) -> Resource {
        let mut attrs = vec![
            KeyValue::new(resource::SERVICE_NAME, self.service_name.clone()),
            KeyValue::new(resource::SERVICE_VERSION, self.service_version.clone()),
            KeyValue::new(resource::SERVICE_NAMESPACE, self.service_namespace.clone()),
        ];

        if !self.environment.is_empty() {
            attrs.push(KeyValue::new(
                resource::DEPLOYMENT_ENVIRONMENT_NAME,
                self.environment.clone(),
            ));
        }

        for (key, value) in &self.attributes {
            attrs.push(KeyValue::new(key.clone(), value.clone()));
        }

        Resource::new(attrs)
    }
}

pub trait ResourceCustomizer: Send + Sync + std::fmt::Debug {
    fn customize(&self, resource: Resource) -> Result<Resource, anyhow::Error>;
}

pub type ResourceFactory = dyn Fn() -> Resource + Send + Sync;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let mut cfg = Config::new("test-service");
        cfg.apply_defaults();

        assert_eq!(cfg.resource.service_name, "test-service");
        assert_eq!(cfg.resource.service_version, DEFAULT_SERVICE_VERSION);
        assert_eq!(cfg.resource.service_namespace, DEFAULT_SERVICE_NAMESPACE);
        assert_eq!(cfg.logger.service_name, "test-service");
        assert_eq!(cfg.tracer.service_name, "test-service");
        assert_eq!(cfg.meter.service_name, "test-service");
        assert_eq!(cfg.profiler.service_name, "test-service");
    }

    #[test]
    fn test_config_validation_requires_service_name() {
        let cfg = Config {
            resource: ResourceConfig {
                service_name: String::new(),
                ..ResourceConfig::new("test")
            },
            ..Config::new("test")
        };

        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::ServiceNameRequired)
        ));
    }

    #[test]
    fn test_resource_builder() {
        let resource_cfg = ResourceConfig::new("my-service")
            .with_version("1.2.3")
            .with_namespace("production")
            .with_environment("prod")
            .with_attribute("custom.key", "custom.value");

        let resource = resource_cfg.build();
        
        assert!(resource.iter().any(|(k, v)| {
            k.as_str() == "service.name" && v.as_str() == "my-service"
        }));
    }
}

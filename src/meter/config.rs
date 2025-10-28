use crate::auth::Credentials;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_EXPORT_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum MeterError {
    #[error("meter service_name is required")]
    ServiceNameRequired,
    #[error("meter endpoint is required when enabled")]
    EndpointRequired,
}

#[derive(Clone, Debug)]
pub struct MeterConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub service_name: String,
    pub export_interval: Duration,
    pub runtime: RuntimeConfig,
    pub credentials: Credentials,
    pub use_global: bool,
}

impl MeterConfig {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            enabled: true,
            endpoint: None,
            service_name: service_name.into(),
            export_interval: DEFAULT_EXPORT_INTERVAL,
            runtime: RuntimeConfig::default(),
            credentials: Credentials::new(),
            use_global: false,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    pub fn with_export_interval(mut self, interval: Duration) -> Self {
        self.export_interval = interval;
        self
    }

    pub fn with_runtime(mut self, runtime: RuntimeConfig) -> Self {
        self.runtime = runtime;
        self
    }

    pub fn with_credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = credentials;
        self
    }

    pub fn use_global(mut self, use_global: bool) -> Self {
        self.use_global = use_global;
        self
    }

    pub fn apply_defaults(&mut self) {
        if self.export_interval.as_secs() == 0 {
            self.export_interval = DEFAULT_EXPORT_INTERVAL;
        }
    }

    pub fn validate(&self) -> Result<(), MeterError> {
        if !self.enabled {
            return Ok(());
        }
        if self.service_name.is_empty() {
            return Err(MeterError::ServiceNameRequired);
        }
        if self.endpoint.is_none() {
            return Err(MeterError::EndpointRequired);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeConfig {
    pub enabled: bool,
}

impl RuntimeConfig {
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meter_config_disabled_passes_validation() {
        let config = MeterConfig::new("test").enabled(false);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_meter_config_enabled_requires_endpoint() {
        let config = MeterConfig::new("test");
        assert!(matches!(
            config.validate(),
            Err(MeterError::EndpointRequired)
        ));
    }

    #[test]
    fn test_meter_config_applies_defaults() {
        let mut config = MeterConfig::new("test");
        config.export_interval = Duration::from_secs(0);

        config.apply_defaults();

        assert_eq!(config.export_interval, DEFAULT_EXPORT_INTERVAL);
    }

    #[test]
    fn test_meter_config_builder() {
        let config = MeterConfig::new("my-service")
            .with_endpoint("http://localhost:9009")
            .with_export_interval(Duration::from_secs(30));

        assert!(config.enabled);
        assert_eq!(config.endpoint.unwrap(), "http://localhost:9009");
        assert_eq!(config.export_interval, Duration::from_secs(30));
    }

    #[test]
    fn test_runtime_config_builder() {
        let runtime = RuntimeConfig::default();
        assert!(!runtime.enabled);

        let runtime = RuntimeConfig::default().enabled(true);
        assert!(runtime.enabled);
    }
}

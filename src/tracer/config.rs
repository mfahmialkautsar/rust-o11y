use crate::auth::Credentials;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_SAMPLE_RATIO: f64 = 1.0;
const DEFAULT_EXPORT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum TracerError {
    #[error("tracer service_name is required")]
    ServiceNameRequired,
    #[error("tracer endpoint is required when enabled")]
    EndpointRequired,
}

#[derive(Clone, Debug)]
pub struct TracerConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub service_name: String,
    pub sample_ratio: f64,
    pub export_timeout: Duration,
    pub credentials: Credentials,
    pub use_global: bool,
}

impl TracerConfig {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            enabled: true,
            endpoint: None,
            service_name: service_name.into(),
            sample_ratio: DEFAULT_SAMPLE_RATIO,
            export_timeout: DEFAULT_EXPORT_TIMEOUT,
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

    pub fn with_sample_ratio(mut self, ratio: f64) -> Self {
        self.sample_ratio = ratio;
        self
    }

    pub fn with_export_timeout(mut self, timeout: Duration) -> Self {
        self.export_timeout = timeout;
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
        if self.sample_ratio <= 0.0 {
            self.sample_ratio = DEFAULT_SAMPLE_RATIO;
        }
        if self.export_timeout.as_secs() == 0 {
            self.export_timeout = DEFAULT_EXPORT_TIMEOUT;
        }
    }

    pub fn validate(&self) -> Result<(), TracerError> {
        if !self.enabled {
            return Ok(());
        }
        if self.service_name.is_empty() {
            return Err(TracerError::ServiceNameRequired);
        }
        if self.endpoint.is_none() {
            return Err(TracerError::EndpointRequired);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracer_config_disabled_passes_validation() {
        let config = TracerConfig::new("test").enabled(false);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_tracer_config_enabled_requires_endpoint() {
        let config = TracerConfig::new("test");
        assert!(matches!(
            config.validate(),
            Err(TracerError::EndpointRequired)
        ));
    }

    #[test]
    fn test_tracer_config_applies_defaults() {
        let mut config = TracerConfig::new("test");
        config.sample_ratio = 0.0;
        config.export_timeout = Duration::from_secs(0);

        config.apply_defaults();

        assert_eq!(config.sample_ratio, DEFAULT_SAMPLE_RATIO);
        assert_eq!(config.export_timeout, DEFAULT_EXPORT_TIMEOUT);
    }

    #[test]
    fn test_tracer_config_builder() {
        let config = TracerConfig::new("my-service")
            .with_endpoint("http://localhost:4317")
            .with_sample_ratio(0.5);

        assert!(config.enabled);
        assert_eq!(config.endpoint.unwrap(), "http://localhost:4317");
        assert_eq!(config.sample_ratio, 0.5);
    }
}

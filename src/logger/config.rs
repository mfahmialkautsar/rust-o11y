use crate::auth::Credentials;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum LoggerError {
    #[error("logger service_name is required")]
    ServiceNameRequired,
    #[error("logger endpoint is required when enabled")]
    EndpointRequired,
}

#[derive(Clone, Debug)]
pub struct LoggerConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub service_name: String,
    pub environment: String,
    pub timeout: Duration,
    pub credentials: Credentials,
}

impl LoggerConfig {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            enabled: true,
            endpoint: None,
            service_name: service_name.into(),
            environment: "development".to_string(),
            timeout: DEFAULT_TIMEOUT,
            credentials: Credentials::new(),
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

    pub fn with_environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = environment.into();
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = credentials;
        self
    }

    pub fn apply_defaults(&mut self) {
        if self.timeout.as_secs() == 0 {
            self.timeout = DEFAULT_TIMEOUT;
        }
        if self.environment.is_empty() {
            self.environment = "development".to_string();
        }
    }

    pub fn validate(&self) -> Result<(), LoggerError> {
        if !self.enabled {
            return Ok(());
        }
        if self.service_name.is_empty() {
            return Err(LoggerError::ServiceNameRequired);
        }
        if self.endpoint.is_none() {
            return Err(LoggerError::EndpointRequired);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_config_disabled_passes_validation() {
        let config = LoggerConfig::new("test").enabled(false);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_logger_config_enabled_requires_endpoint() {
        let config = LoggerConfig::new("test");
        assert!(matches!(
            config.validate(),
            Err(LoggerError::EndpointRequired)
        ));
    }

    #[test]
    fn test_logger_config_builder() {
        let config = LoggerConfig::new("my-service")
            .with_endpoint("http://localhost:3100")
            .with_environment("production");

        assert!(config.enabled);
        assert_eq!(config.endpoint.unwrap(), "http://localhost:3100");
        assert_eq!(config.environment, "production");
    }
}

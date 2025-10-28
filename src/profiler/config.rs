use crate::auth::Credentials;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfilerError {
    #[error("profiler service_name is required")]
    ServiceNameRequired,
    #[error("profiler server_url is required when enabled")]
    ServerUrlRequired,
}

#[derive(Clone, Debug)]
pub struct ProfilerConfig {
    pub enabled: bool,
    pub server_url: Option<String>,
    pub service_name: String,
    pub tags: HashMap<String, String>,
    pub tenant_id: Option<String>,
    pub credentials: Credentials,
}

impl ProfilerConfig {
    pub fn new(service_name: impl Into<String>) -> Self {
        let service_name = service_name.into();
        let mut tags = HashMap::new();
        tags.insert("service".to_string(), service_name.clone());
        tags.insert("service_name".to_string(), service_name.clone());

        Self {
            enabled: true,
            server_url: None,
            service_name,
            tags,
            tenant_id: Some("anonymous".to_string()),
            credentials: Credentials::new(),
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_server_url(mut self, url: impl Into<String>) -> Self {
        self.server_url = Some(url.into());
        self
    }

    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn with_credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = credentials;
        self
    }

    pub fn apply_defaults(&mut self) {
        if self.tenant_id.is_none() {
            self.tenant_id = Some("anonymous".to_string());
        }

        if !self.tags.contains_key("service") {
            self.tags
                .insert("service".to_string(), self.service_name.clone());
        }
        if !self.tags.contains_key("service_name") {
            self.tags
                .insert("service_name".to_string(), self.service_name.clone());
        }
    }

    pub fn validate(&self) -> Result<(), ProfilerError> {
        if !self.enabled {
            return Ok(());
        }
        if self.service_name.is_empty() {
            return Err(ProfilerError::ServiceNameRequired);
        }
        if self.server_url.is_none() {
            return Err(ProfilerError::ServerUrlRequired);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_config_disabled_passes_validation() {
        let config = ProfilerConfig::new("test").enabled(false);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_profiler_config_enabled_requires_server_url() {
        let config = ProfilerConfig::new("test");
        assert!(matches!(
            config.validate(),
            Err(ProfilerError::ServerUrlRequired)
        ));
    }

    #[test]
    fn test_profiler_config_default_tags() {
        let config = ProfilerConfig::new("my-service");
        assert_eq!(config.tags.get("service").unwrap(), "my-service");
        assert_eq!(config.tags.get("service_name").unwrap(), "my-service");
    }

    #[test]
    fn test_profiler_config_builder() {
        let config = ProfilerConfig::new("my-service")
            .with_server_url("http://localhost:4040")
            .with_tag("environment", "production")
            .with_tenant_id("my-tenant");

        assert!(config.enabled);
        assert_eq!(config.server_url.unwrap(), "http://localhost:4040");
        assert_eq!(config.tags.get("environment").unwrap(), "production");
        assert_eq!(config.tenant_id.unwrap(), "my-tenant");
    }
}

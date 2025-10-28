mod config;

pub use config::{LoggerConfig, LoggerError};

use anyhow::Result;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    logs::{BatchLogProcessor, LoggerProvider as SdkLoggerProvider},
    resource::Resource,
};

pub type LoggerProvider = SdkLoggerProvider;

pub fn setup(config: &LoggerConfig, resource: &Resource) -> Result<Option<LoggerProvider>> {
    if !config.enabled {
        return Ok(None);
    }

    let endpoint = config
        .endpoint
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("logger endpoint is required when enabled"))?;

    let normalized_endpoint = if endpoint.ends_with("/v1/logs") {
        endpoint.clone()
    } else {
        format!("{}/v1/logs", endpoint.trim_end_matches('/'))
    };

    let exporter_builder = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_endpoint(normalized_endpoint);

    let exporter = exporter_builder.build()?;
    let processor = BatchLogProcessor::builder(exporter, opentelemetry_sdk::runtime::Tokio).build();

    let provider = SdkLoggerProvider::builder()
        .with_resource(resource.clone())
        .with_log_processor(processor)
        .build();

    Ok(Some(provider))
}

pub fn shutdown(provider: LoggerProvider) {
    if let Err(e) = provider.shutdown() {
        eprintln!("failed to shut down logger provider: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_logger() {
        let config = LoggerConfig::new("test-service");
        let resource = Resource::default();

        let result = setup(&config, &resource).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_enabled_logger_requires_endpoint() {
        let mut config = LoggerConfig::new("test-service");
        config.enabled = true;
        let resource = Resource::default();

        let result = setup(&config, &resource);
        assert!(result.is_err());
    }
}

mod config;
mod runtime;

pub use config::{MeterConfig, MeterError, RuntimeConfig};
pub use runtime::register_runtime_metrics;

use anyhow::Result;
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{metrics::SdkMeterProvider, resource::Resource};

pub type MeterProvider = SdkMeterProvider;

pub fn setup(config: &MeterConfig, resource: &Resource) -> Result<Option<MeterProvider>> {
    if !config.enabled {
        return Ok(None);
    }

    let endpoint = config
        .endpoint
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("meter endpoint is required when enabled"))?;

    let normalized_endpoint = if endpoint.ends_with("/v1/metrics") {
        endpoint.clone()
    } else {
        format!("{}/v1/metrics", endpoint.trim_end_matches('/'))
    };

    let exporter_builder = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_endpoint(normalized_endpoint);

    let exporter = exporter_builder.build()?;

    let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(
        exporter,
        opentelemetry_sdk::runtime::Tokio,
    )
    .build();

    let provider = SdkMeterProvider::builder()
        .with_resource(resource.clone())
        .with_reader(reader)
        .build();

    Ok(Some(provider))
}

pub fn init(config: &MeterConfig, resource: &Resource) -> Result<Option<MeterProvider>> {
    let provider = setup(config, resource)?;

    if config.use_global {
        if let Some(ref p) = provider {
            global::set_meter_provider(p.clone());
        }
    }

    Ok(provider)
}

pub fn shutdown(provider: MeterProvider) {
    if let Err(e) = provider.shutdown() {
        eprintln!("failed to shut down meter provider: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_meter() {
        let config = MeterConfig::new("test-service");
        let resource = Resource::default();

        let result = setup(&config, &resource).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_enabled_meter_requires_endpoint() {
        let mut config = MeterConfig::new("test-service");
        config.enabled = true;
        let resource = Resource::default();

        let result = setup(&config, &resource);
        assert!(result.is_err());
    }

    #[test]
    fn test_endpoint_normalization() {
        let config = MeterConfig::new("test-service")
            .enabled(true)
            .with_endpoint("http://localhost:9009");

        assert!(config.endpoint.unwrap().contains("9009"));
    }
}

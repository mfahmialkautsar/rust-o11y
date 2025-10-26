use std::sync::OnceLock;

use anyhow::{Context, Result};
use opentelemetry::{
    KeyValue, global,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{metrics::SdkMeterProvider, resource::Resource};

use crate::{config::AppConfig};

pub type MeterProvider = SdkMeterProvider;

const METER_NAME: &str = "rust-clean-arch";

static COMMON_ATTRIBUTES: OnceLock<Vec<KeyValue>> = OnceLock::new();

pub fn init(config: &AppConfig, resource: &Resource) -> Result<Option<MeterProvider>> {
    let Some(base_url) = config.mimir_push_url.as_ref() else {
        return Ok(None);
    };

    let trimmed = base_url.trim_end_matches('/');
    let endpoint = if trimmed.ends_with("/v1/metrics") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1/metrics")
    };

    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .with_context(|| format!("failed to build OTLP metric exporter for {}", base_url))?;

    let provider = SdkMeterProvider::builder()
        .with_resource(resource.clone())
        .with_periodic_exporter(exporter)
        .build();

    global::set_meter_provider(provider.clone());

    init_common_attributes(config);

    Ok(Some(provider))
}

pub fn shutdown(provider: MeterProvider) {
    if let Err(error) = provider.shutdown() {
        eprintln!("failed to shut down meter provider: {error:?}");
    }
}

fn init_common_attributes(config: &AppConfig) {
    let attributes = vec![
        KeyValue::new("service.name", config.service_name.clone()),
        KeyValue::new("service", config.service_name.clone()),
    ];
    let _ = COMMON_ATTRIBUTES.set(attributes);
}

fn common_attributes() -> &'static [KeyValue] {
    COMMON_ATTRIBUTES.get().map(Vec::as_slice).unwrap_or(&[])
}

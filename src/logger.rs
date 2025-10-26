use anyhow::{Context, Result};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    logs::{BatchLogProcessor, SdkLoggerProvider},
    resource::Resource,
};

use crate::config::AppConfig;

pub type LoggerProvider = SdkLoggerProvider;

pub fn init(config: &AppConfig, resource: &Resource) -> Result<Option<LoggerProvider>> {
    let loki_url = match &config.loki_url {
        Some(url) => url,
        None => return Ok(None),
    };

    let endpoint = format!("{}/otlp/v1/logs", loki_url.trim_end_matches('/'));

    let exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .with_context(|| format!("failed to build OTLP log exporter for {loki_url}"))?;

    let processor = BatchLogProcessor::builder(exporter).build();

    let provider = SdkLoggerProvider::builder()
        .with_resource(resource.clone())
        .with_log_processor(processor)
        .build();

    Ok(Some(provider))
}

pub fn shutdown(provider: LoggerProvider) {
    if let Err(error) = provider.shutdown() {
        eprintln!("failed to shut down logger provider: {error:?}");
    }
}

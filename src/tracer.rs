use anyhow::{Context, Result};
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    resource::Resource,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};

use crate::config::AppConfig;

pub type TracerProvider = SdkTracerProvider;

pub fn init(config: &AppConfig, resource: &Resource) -> Result<Option<TracerProvider>> {
    if let Some(endpoint) = &config.tempo_endpoint {
        global::set_text_map_propagator(TraceContextPropagator::new());

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint.clone())
            .build()
            .with_context(|| format!("failed to build OTLP span exporter for {endpoint}"))?;

        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource.clone())
            .with_id_generator(RandomIdGenerator::default())
            .with_sampler(Sampler::AlwaysOn)
            .build();

        global::set_tracer_provider(provider.clone());

        Ok(Some(provider))
    } else {
        Ok(None)
    }
}

pub fn shutdown(provider: TracerProvider) {
    if let Err(error) = provider.shutdown() {
        eprintln!("failed to shut down tracer provider: {error:?}");
    }
}

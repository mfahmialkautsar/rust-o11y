mod config;

pub use config::{TracerConfig, TracerError};

use anyhow::Result;
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    resource::Resource,
    trace::{RandomIdGenerator, Sampler, TracerProvider as SdkTracerProvider},
};

pub type TracerProvider = SdkTracerProvider;

pub fn setup(config: &TracerConfig, resource: &Resource) -> Result<Option<TracerProvider>> {
    if !config.enabled {
        return Ok(None);
    }

    let endpoint = config
        .endpoint
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("tracer endpoint is required when enabled"))?;

    global::set_text_map_propagator(TraceContextPropagator::new());

    let exporter_builder = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.clone());

    let exporter = exporter_builder.build()?;

    let sampler = sampler_from_ratio(config.sample_ratio);

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(resource.clone())
        .with_id_generator(RandomIdGenerator::default())
        .with_sampler(sampler)
        .build();

    Ok(Some(provider))
}

pub fn init(config: &TracerConfig, resource: &Resource) -> Result<Option<TracerProvider>> {
    let provider = setup(config, resource)?;

    if config.use_global {
        if let Some(ref p) = provider {
            global::set_tracer_provider(p.clone());
        }
    }

    Ok(provider)
}

pub fn shutdown(provider: TracerProvider) {
    if let Err(e) = provider.shutdown() {
        eprintln!("failed to shut down tracer provider: {e:?}");
    }
}

fn sampler_from_ratio(ratio: f64) -> Sampler {
    if ratio <= 0.0 {
        Sampler::AlwaysOff
    } else if ratio >= 1.0 {
        Sampler::AlwaysOn
    } else {
        Sampler::TraceIdRatioBased(ratio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_tracer() {
        let config = TracerConfig::new("test-service");
        let resource = Resource::default();

        let result = setup(&config, &resource).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_sampler_ratios() {
        assert!(matches!(sampler_from_ratio(-1.0), Sampler::AlwaysOff));
        assert!(matches!(sampler_from_ratio(0.0), Sampler::AlwaysOff));
        assert!(matches!(sampler_from_ratio(1.0), Sampler::AlwaysOn));
        assert!(matches!(sampler_from_ratio(2.0), Sampler::AlwaysOn));

        match sampler_from_ratio(0.5) {
            Sampler::TraceIdRatioBased(r) => assert_eq!(r, 0.5),
            _ => panic!("expected TraceIdRatioBased sampler"),
        }
    }

    #[test]
    fn test_enabled_tracer_requires_endpoint() {
        let mut config = TracerConfig::new("test-service");
        config.enabled = true;
        let resource = Resource::default();

        let result = setup(&config, &resource);
        assert!(result.is_err());
    }
}

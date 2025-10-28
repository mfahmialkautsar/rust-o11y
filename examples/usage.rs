use anyhow::Result;
use o11y::Credentials;
use o11y::logger::LoggerConfig;
use o11y::meter::{MeterConfig, RuntimeConfig};
use o11y::profiler::ProfilerConfig;
use o11y::tracer::TracerConfig;
use o11y::{Config, Telemetry};
use std::time::Duration;

fn main() -> Result<()> {
    // Example 1: All-in-one with all components enabled
    let config = Config::new("example-service")
        .with_logger(
            LoggerConfig::new("example-service")
                .with_endpoint("http://localhost:3100")
                .with_environment("development"),
        )
        .with_tracer(
            TracerConfig::new("example-service")
                .with_endpoint("http://localhost:4317")
                .with_sample_ratio(1.0)
                .use_global(true),
        )
        .with_meter(
            MeterConfig::new("example-service")
                .with_endpoint("http://localhost:9009")
                .with_export_interval(Duration::from_secs(10))
                .with_runtime(RuntimeConfig::default().enabled(true)),
        )
        .with_profiler(
            ProfilerConfig::new("example-service")
                .with_server_url("http://localhost:4040")
                .with_tag("environment", "development"),
        );

    let tele = Telemetry::new(config)?;

    println!("✓ Logger: {}", tele.has_logger());
    println!("✓ Tracer: {}", tele.has_tracer());
    println!("✓ Meter: {}", tele.has_meter());
    println!("✓ Profiler: {}", tele.has_profiler());

    // Your application logic here...

    // Shutdown
    tele.shutdown();

    Ok(())
}

// Example 2: With authentication
fn _example_with_auth() -> Result<()> {
    let creds = Credentials::new().with_bearer("my-secret-token");

    let config = Config::new("auth-service")
        .with_logger(
            LoggerConfig::new("auth-service")
                .with_endpoint("http://localhost:3100")
                .with_credentials(creds.clone()),
        )
        .with_tracer(
            TracerConfig::new("auth-service")
                .with_endpoint("http://localhost:4317")
                .with_credentials(creds),
        );

    let tele = Telemetry::new(config)?;
    tele.shutdown();
    Ok(())
}

// Example 3: Standalone component
fn _example_standalone() -> Result<()> {
    use o11y::ResourceConfig;
    use o11y::tracer;

    let resource = ResourceConfig::new("standalone-service")
        .with_version("1.0.0")
        .with_environment("production")
        .build();

    let tracer_config = TracerConfig::new("standalone-service")
        .enabled(true)
        .with_endpoint("http://localhost:4317")
        .use_global(true);

    let tracer_provider = tracer::init(&tracer_config, &resource)?;

    // Use tracer...

    if let Some(provider) = tracer_provider {
        tracer::shutdown(provider);
    }

    Ok(())
}

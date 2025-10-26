# o11y - Rust Observability Package

OpenTelemetry instrumentation for Rust applications with unified configuration and flexible deployment options.

## Features

- **All-in-one or standalone** - Use `Telemetry` for coordinated setup or initialize components individually
- **Config-based** - No environment variables, all configuration via structs
- **Multiple auth methods** - BasicAuth, Bearer tokens, API keys, custom headers per component
- **Resource management** - Automatic resource building with service identification
- **Validation** - Config validation with clear error messages
- **Runtime metrics** - Optional Tokio runtime metrics for meters
- **Cargo features** - Enable only the observability components you need
- **OTLP native** - Uses OpenTelemetry Protocol for all backends (Grafana OTLP endpoints)
- **Cross-platform** - Profiler support on Unix platforms

## Cargo Features

By default, all features are enabled. You can selectively enable only what you need:

```toml
[dependencies]
o11y = { version = "*", default-features = false, features = ["logger", "tracer"] }
```

Available features:
- `logger` - Logging via OTLP
- `tracer` - Distributed tracing
- `meter` - Metrics collection
- `profiler` - Continuous profiling (Unix only)

## Installation

```toml
[dependencies]
o11y = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

### All-in-One Setup

```rust
use o11y::{Config, Telemetry};
use o11y::logger::LoggerConfig;
use o11y::tracer::TracerConfig;
use o11y::meter::MeterConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new("my-service")
        .with_logger(
            LoggerConfig::new("my-service")
                .enabled(true)
                .with_endpoint("http://localhost:3100/otlp")
        )
        .with_tracer(
            TracerConfig::new("my-service")
                .enabled(true)
                .with_endpoint("http://localhost:4317")
        )
        .with_meter(
            MeterConfig::new("my-service")
                .enabled(true)
                .with_endpoint("http://localhost:9009/otlp")
        );

    let telemetry = Telemetry::setup(&config)?;
    
    // Your application code here
    
    telemetry.shutdown()?;
    Ok(())
}
```

### Standalone Component Setup

```rust
use o11y::logger::{self, LoggerConfig};
use o11y::ResourceConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resource = ResourceConfig::new("my-service")
        .with_version("1.0.0")
        .with_environment("production")
        .build();

    let logger_config = LoggerConfig::new("my-service")
        .enabled(true)
        .with_endpoint("http://localhost:3100/otlp");

    let logger_provider = logger::setup(&logger_config, &resource)?;
    
    // Use logger
    
    logger::shutdown(logger_provider)?;
    Ok(())
}
```

## Configuration

### Logger Configuration

```rust
use o11y::logger::LoggerConfig;

let logger_cfg = LoggerConfig::new("my-service")
    .enabled(true)
    .with_endpoint("http://localhost:3100/otlp")
    .with_environment("production");
```

**Note**: Logger endpoint should include `/otlp` path for Grafana Loki OTLP endpoint.

### Tracer Configuration

```rust
use o11y::tracer::TracerConfig;
use std::time::Duration;

let tracer_cfg = TracerConfig::new("my-service")
    .enabled(true)
    .with_endpoint("http://localhost:4317")
    .with_sample_ratio(0.1)  // Sample 10% of traces
    .with_export_timeout(Duration::from_secs(30));
```

**Note**: Tracer uses gRPC endpoint (default port 4317 for Grafana Tempo).

### Meter Configuration

```rust
use o11y::meter::{MeterConfig, RuntimeConfig};
use std::time::Duration;

let meter_cfg = MeterConfig::new("my-service")
    .enabled(true)
    .with_endpoint("http://localhost:9009/otlp")
    .with_export_interval(Duration::from_secs(60))
    .with_runtime(RuntimeConfig::default().enabled(true));
```

**Note**: Meter endpoint should include `/otlp` path for Grafana Mimir OTLP endpoint (default port 9009).

### Profiler Configuration

```rust
use o11y::profiler::ProfilerConfig;
use std::time::Duration;

#[cfg(unix)]
let profiler_cfg = ProfilerConfig::new("my-service")
    .enabled(true)
    .with_endpoint("http://localhost:4040")
    .with_sample_rate(100)  // Hz
    .with_upload_interval(Duration::from_secs(15));
```

### Resource Configuration

```rust
use o11y::ResourceConfig;

let resource = ResourceConfig::new("my-service")
    .with_version("1.0.0")
    .with_environment("production")
    .with_namespace("backend")
    .with_tenant_id("my-tenant");
```

## Global Providers

All enabled components are automatically registered with OpenTelemetry's global registry:
- Tracer provider → `opentelemetry::global::set_tracer_provider()`
- Meter provider → `opentelemetry::global::set_meter_provider()`

This means you can use the standard OpenTelemetry APIs throughout your application:

```rust
use opentelemetry::global;

// Get global tracer
let tracer = global::tracer("my-component");

// Get global meter
let meter = global::meter("my-component");
```

## Authentication

Multiple authentication methods are supported per component:

### Basic Auth

```rust
use o11y::auth::Credentials;

let creds = Credentials::basic_auth("username", "password");

let logger_cfg = LoggerConfig::new("my-service")
    .enabled(true)
    .with_endpoint("http://localhost:3100/otlp")
    .with_credentials(creds);
```

### Bearer Token

```rust
let creds = Credentials::bearer_token("my-token");
```

### API Key

```rust
let creds = Credentials::api_key("X-API-Key", "my-api-key");
```

### Custom Headers

```rust
let creds = Credentials::custom_header("X-Custom-Auth", "value");
```

## Examples

See the [examples](examples/) directory for complete working examples:

```bash
cargo run --example usage
```

## Backends

Designed for Grafana observability stack:
- **Logger** → Grafana Loki (port 3100, `/otlp` path)
- **Tracer** → Grafana Tempo (gRPC port 4317)
- **Meter** → Grafana Mimir (port 9009, `/otlp` path)
- **Profiler** → Pyroscope (port 4040)

## Error Handling

All operations return `Result<T, o11y::Error>`:

```rust
match Telemetry::setup(&config) {
    Ok(telemetry) => { /* success */ }
    Err(o11y::Error::Config(msg)) => { /* config validation error */ }
    Err(o11y::Error::Logger(msg)) => { /* logger setup error */ }
    Err(o11y::Error::Tracer(msg)) => { /* tracer setup error */ }
    Err(o11y::Error::Meter(msg)) => { /* meter setup error */ }
    Err(o11y::Error::Profiler(msg)) => { /* profiler setup error */ }
    Err(e) => { /* other errors */ }
}
```

## Testing

```bash
# Unit tests
cargo test --lib

# All tests
cargo test

# With specific features
cargo test --no-default-features --features logger,tracer
```

## License

[License information]

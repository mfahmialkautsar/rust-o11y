# o11y

![CI](https://github.com/mfahmialkautsar/rust-o11y/actions/workflows/ci.yml/badge.svg)
![Docs](https://github.com/mfahmialkautsar/rust-o11y/actions/workflows/docs.yml/badge.svg)
[![codecov](https://codecov.io/gh/mfahmialkautsar/rust-o11y/graph/badge.svg)](https://codecov.io/gh/mfahmialkautsar/rust-o11y)
[![Crates.io](https://img.shields.io/crates/v/o11y.svg)](https://crates.io/crates/o11y)
[![Docs.rs](https://img.shields.io/docsrs/o11y)](https://docs.rs/o11y)
![MSRV](https://img.shields.io/badge/rustc-2024%20edition-orange.svg)
[![License](https://img.shields.io/github/license/mfahmialkautsar/rust-o11y)](LICENSE)

Observability building blocks for Rust services: unified configuration for logging, tracing, metrics, and continuous profiling on top of OpenTelemetry.

## Highlights

- **Unified bootstrap** – `Telemetry::new` wires logger, tracer, meter, and profiler from one config.
- **Modular features** – enable only the components you need via Cargo features.
- **Credential helpers** – convenience constructors for basic auth, bearer tokens, API keys, and custom headers.
- **Resource defaults** – consistent service metadata with optional environment overrides.
- **Tokio runtime metrics** – optional gauges for runtime worker state when meters are global.

## Installation

```bash
cargo add o11y --features="logger tracer meter profiler"
```

The crate targets the 2024 edition. Tokio is required when using async components:

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
```

## Quick Start

```rust
use o11y::{Config, Telemetry};
use o11y::logger::LoggerConfig;
use o11y::meter::{MeterConfig, RuntimeConfig};
use o11y::tracer::TracerConfig;

fn main() -> anyhow::Result<()> {
    let config = Config::new("billing-service")
        .with_logger(
            LoggerConfig::new("billing-service")
                .with_endpoint("http://localhost:3100/otlp"),
        )
        .with_tracer(
            TracerConfig::new("billing-service")
                .with_endpoint("http://localhost:4317")
                .use_global(true),
        )
        .with_meter(
            MeterConfig::new("billing-service")
                .with_endpoint("http://localhost:9009/otlp")
                .with_runtime(RuntimeConfig::default())
                .use_global(true),
        );

    let telemetry = Telemetry::new(config)?;
    // Emit logs, traces, and metrics using OpenTelemetry APIs here.
    telemetry.shutdown();
    Ok(())
}
```

See `tests/telemetry_all_in_one.rs` for an end-to-end example that exercises logs, traces, and metrics against Grafana backends.

## Feature Flags

| Feature | Description |
|---------|-------------|
| `logger` | OTLP logging with Loki-compatible exporters |
| `tracer` | Distributed tracing via OTLP/Tempo |
| `meter` | Metrics export with optional Tokio runtime stats |
| `profiler` | Pyroscope integration (Unix only) |

```toml
[dependencies]
o11y = { version = "0.0.1", default-features = false, features = ["logger", "tracer"] }
```

## Configuration Overview

- `ResourceConfig` controls service metadata (name, version, namespace, environment).
- Component configs (`LoggerConfig`, `TracerConfig`, `MeterConfig`, `ProfilerConfig`) expose builder-style APIs for endpoints, auth, timeouts, sampling, and runtime behavior.
- Authentication helpers live in `o11y::auth::Credentials`.
- Global registration is optional per component; use `use_global(true)` to apply providers to OpenTelemetry globals.

Refer to module-level docs on [docs.rs](https://docs.rs/o11y) for the complete API surface.

## Testing

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Integration scenarios mirror README samples:

```bash
cargo test --test telemetry_all_in_one
cargo test --test telemetry_standalone
```

## License

This project is licensed under the [Apache-2.0](LICENSE).

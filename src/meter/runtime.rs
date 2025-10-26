use opentelemetry::global;
use tokio::runtime::Handle;

pub fn register_runtime_metrics(meter_name: String) -> Result<(), Box<dyn std::error::Error>> {
    let meter = global::meter(Box::leak(meter_name.into_boxed_str()) as &'static str);

    let _num_workers = meter
        .u64_observable_gauge("tokio.runtime.workers")
        .with_description("Number of worker threads in the runtime")
        .with_callback(move |observer| {
            if let Ok(handle) = Handle::try_current() {
                let metrics = handle.metrics();
                observer.observe(metrics.num_workers() as u64, &[]);
            }
        })
        .build();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_runtime_metrics() {
        let result = register_runtime_metrics("test_meter".to_string());
        assert!(result.is_ok());
    }
}

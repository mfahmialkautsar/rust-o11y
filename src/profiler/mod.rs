mod config;

pub use config::{ProfilerConfig, ProfilerError};

#[cfg(unix)]
use anyhow::{Context, Result};
#[cfg(unix)]
use pyroscope::pyroscope::{PyroscopeAgent as Agent, PyroscopeAgentRunning};
#[cfg(unix)]
use pyroscope_pprofrs::{pprof_backend, PprofConfig};

#[cfg(unix)]
pub type PyroscopeAgent = Agent<PyroscopeAgentRunning>;

#[cfg(not(unix))]
pub type PyroscopeAgent = ();

#[cfg(unix)]
pub fn setup(config: &ProfilerConfig) -> Result<Option<PyroscopeAgent>> {
    if !config.enabled {
        return Ok(None);
    }

    let server_url = config
        .server_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("profiler server_url is required when enabled"))?;

    let backend = pprof_backend(PprofConfig::default());

    let mut agent_builder = Agent::builder(server_url, &config.service_name).backend(backend);

    if !config.tags.is_empty() {
        let tags: Vec<(&str, &str)> = config
            .tags
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        agent_builder = agent_builder.tags(tags);
    }

    if let Some(ref tenant_id) = config.tenant_id {
        agent_builder = agent_builder.tenant_id(tenant_id.clone());
    }

    if let Some((username, password)) = config.credentials.basic_auth() {
        agent_builder = agent_builder.basic_auth(username, password);
    }

    let agent = agent_builder
        .build()
        .context("failed to configure pyroscope agent")?;

    let running = agent
        .start()
        .context("failed to start pyroscope agent")?;

    Ok(Some(running))
}

#[cfg(not(unix))]
pub fn setup(_config: &ProfilerConfig) -> anyhow::Result<Option<PyroscopeAgent>> {
    Ok(None)
}

#[cfg(unix)]
pub fn init(config: &ProfilerConfig) -> Result<Option<PyroscopeAgent>> {
    setup(config)
}

#[cfg(not(unix))]
pub fn init(_config: &ProfilerConfig) -> anyhow::Result<Option<PyroscopeAgent>> {
    Ok(None)
}

#[cfg(unix)]
pub fn shutdown(agent: PyroscopeAgent) {
    if let Err(e) = agent.stop() {
        eprintln!("failed to shut down pyroscope agent: {e:?}");
    }
}

#[cfg(not(unix))]
pub fn shutdown(_agent: PyroscopeAgent) {}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_profiler() {
        let config = ProfilerConfig::new("test-service");
        let result = setup(&config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_enabled_profiler_requires_server_url() {
        let mut config = ProfilerConfig::new("test-service");
        config.enabled = true;
        let result = setup(&config);
        assert!(result.is_err());
    }
}

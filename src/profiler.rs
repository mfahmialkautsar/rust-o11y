#[cfg(unix)]
use anyhow::{Context, Result};
#[cfg(unix)]
use pyroscope::pyroscope::{PyroscopeAgent as Agent, PyroscopeAgentRunning};
#[cfg(unix)]
use pyroscope_pprofrs::{PprofConfig, pprof_backend};

use crate::config::AppConfig;

#[cfg(unix)]
pub type PyroscopeAgent = Agent<PyroscopeAgentRunning>;

#[cfg(not(unix))]
pub type PyroscopeAgent = ();

#[cfg(unix)]
pub fn init(config: &AppConfig) -> Result<Option<PyroscopeAgent>> {
    if let Some(pyroscope_url) = &config.pyroscope_url {
        let backend = pprof_backend(PprofConfig::default());
        let agent = Agent::builder(pyroscope_url, &config.service_name)
            .backend(backend)
            .build()
            .context("failed to configure pyroscope agent")?;

        let running_agent = agent.start().context("failed to start pyroscope agent")?;

        Ok(Some(running_agent))
    } else {
        Ok(None)
    }
}

#[cfg(not(unix))]
pub fn init(_config: &AppConfig) -> anyhow::Result<Option<PyroscopeAgent>> {
    Ok(None)
}

#[cfg(unix)]
pub fn shutdown(agent: PyroscopeAgent) {
    if let Err(e) = agent.stop() {
        eprintln!("Error shutting down pyroscope agent: {:?}", e);
    }
}

#[cfg(not(unix))]
pub fn shutdown(_agent: PyroscopeAgent) {}

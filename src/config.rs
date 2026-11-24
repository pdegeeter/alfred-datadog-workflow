//! Workflow configuration, read from Alfred's workflow variables (which reach
//! the binary as environment variables).

use std::path::PathBuf;

use anyhow::{Context, Result};

/// Default Datadog site when `DD_SITE` is unset or blank.
pub const DEFAULT_SITE: &str = "datadoghq.eu";

/// Default environment when `DD_ENV` is unset or blank.
pub const DEFAULT_ENV: &str = "production";

/// Everything the workflow needs to know about its surroundings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Datadog site, e.g. `datadoghq.eu`. Used by pup and for every URL.
    pub site: String,
    /// Default environment for services, logs, traces and monitors.
    pub env: String,
    /// `$alfred_workflow_cache`: one JSON file per source.
    pub cache_dir: PathBuf,
    /// `$alfred_workflow_data`: frecency.
    pub data_dir: PathBuf,
}

impl Config {
    /// Read the configuration of the running workflow.
    pub fn from_env() -> Result<Config> {
        let cache_dir = powerpack::env::try_workflow_cache_or_default()
            .context("cannot locate the workflow cache directory (HOME is not set)")?;
        let data_dir = powerpack::env::try_workflow_data_or_default()
            .context("cannot locate the workflow data directory (HOME is not set)")?;

        Ok(Config::from_vars(
            |name| std::env::var(name).ok(),
            cache_dir,
            data_dir,
        ))
    }

    /// Build a configuration from a variable lookup; blank values fall back
    /// to the defaults.
    pub fn from_vars(
        lookup: impl Fn(&str) -> Option<String>,
        cache_dir: PathBuf,
        data_dir: PathBuf,
    ) -> Config {
        let value = |name: &str, default: &str| {
            lookup(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| default.to_owned())
        };

        Config {
            site: value("DD_SITE", DEFAULT_SITE),
            env: value("DD_ENV", DEFAULT_ENV),
            cache_dir,
            data_dir,
        }
    }
}

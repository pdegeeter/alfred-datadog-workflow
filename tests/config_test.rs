//! Tests for reading the workflow configuration.

use std::collections::HashMap;
use std::path::PathBuf;

use alfred_dd::config::Config;

fn config(vars: &[(&str, &str)]) -> Config {
    let vars: HashMap<String, String> = vars
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();

    Config::from_vars(
        |name| vars.get(name).cloned(),
        PathBuf::from("/cache"),
        PathBuf::from("/data"),
    )
}

#[test]
fn defaults_to_the_eu_site_and_production() {
    let config = config(&[]);

    assert_eq!(config.site, "datadoghq.eu");
    assert_eq!(config.env, "production");
}

#[test]
fn reads_site_and_env_from_the_workflow_variables() {
    let config = config(&[("DD_SITE", "datadoghq.com"), ("DD_ENV", "prod")]);

    assert_eq!(config.site, "datadoghq.com");
    assert_eq!(config.env, "prod");
}

#[test]
fn blank_values_fall_back_to_the_defaults() {
    let config = config(&[("DD_SITE", "  "), ("DD_ENV", "")]);

    assert_eq!(config.site, "datadoghq.eu");
    assert_eq!(config.env, "production");
}

#[test]
fn values_are_trimmed() {
    assert_eq!(config(&[("DD_ENV", " staging ")]).env, "staging");
}

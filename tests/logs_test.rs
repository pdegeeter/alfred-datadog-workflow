//! Tests for `ddl`: the Log Explorer link and `service:` completions.

mod common;

use alfred_dd::alfred::duration;
use alfred_dd::cache::{Entry, Key, Kind, Store};
use alfred_dd::filters;
use alfred_dd::services::{self, Service};
use common::{app, fixture, items_json, FakeRunner, NOW_MS};
use serde_json::Value;

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;

/// Seed the services cache of `env` with the fixtures' merged services.
fn seed_services(dir: &std::path::Path, env: &str) {
    let services: Vec<Service> = services::merge(
        services::parse_apm(&fixture("apm_services")).unwrap(),
        services::parse_definitions(&fixture("service_definitions")).unwrap(),
    );
    Store::new(dir.join("cache"))
        .write(
            Kind::Services,
            &Entry {
                key: Key {
                    site: "datadoghq.eu".to_owned(),
                    scope: env.to_owned(),
                },
                fetched_ms: NOW_MS - 3 * HOUR_MS,
                items: services,
            },
        )
        .unwrap();
}

fn run(dir: &std::path::Path, input: &str) -> (Value, FakeRunner) {
    let runner = FakeRunner::new();
    let listing = filters::logs(&app(&runner, dir), input);
    assert!(!listing.rerun);

    (items_json(listing.items), runner)
}

#[test]
fn builds_the_log_explorer_link_without_any_call() {
    let dir = tempfile::tempdir().unwrap();
    let (items, runner) = run(dir.path(), "service:checkout-api status:error timeout -15m");
    let item = &items[0];
    let from = NOW_MS - 15 * MINUTE_MS;

    assert!(runner.calls().is_empty());
    assert_eq!(
        item["title"],
        "Logs: service:checkout-api status:error timeout env:production"
    );
    assert_eq!(
        item["subtitle"],
        "Last 15m · ↩ Log Explorer · ⌘ Traces Explorer · ⌥ copy URL"
    );
    assert_eq!(
        item["arg"],
        format!("open-url https://app.datadoghq.eu/logs?query=service%3Acheckout-api%20status%3Aerror%20timeout%20env%3Aproduction&from_ts={from}&to_ts={NOW_MS}&live=true")
    );
    assert_eq!(
        item["mods"]["cmd"]["arg"],
        format!("open-url https://app.datadoghq.eu/apm/traces?query=service%3Acheckout-api%20status%3Aerror%20timeout%20env%3Aproduction&start={from}&end={NOW_MS}&paused=false")
    );
    assert_eq!(
        item["mods"]["alt"]["arg"],
        format!("copy https://app.datadoghq.eu/logs?query=service%3Acheckout-api%20status%3Aerror%20timeout%20env%3Aproduction&from_ts={from}&to_ts={NOW_MS}&live=true")
    );
}

#[test]
fn an_empty_query_shows_the_env_over_the_last_hour() {
    let dir = tempfile::tempdir().unwrap();
    let (items, _) = run(dir.path(), "");

    assert_eq!(items[0]["title"], "Logs: env:production");
    assert!(items[0]["subtitle"]
        .as_str()
        .unwrap()
        .starts_with("Last 1h"));
}

#[test]
fn env_overrides_the_default() {
    let dir = tempfile::tempdir().unwrap();
    let (items, _) = run(dir.path(), "env:staging timeout");

    assert_eq!(items[0]["title"], "Logs: timeout env:staging");
}

#[test]
fn typing_a_service_lists_matching_services_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    seed_services(dir.path(), "production");

    let (items, runner) = run(dir.path(), "status:error service:pay -4h");

    assert!(runner.calls().is_empty());
    let completions: Vec<(&str, &str)> = items.as_array().unwrap()[1..]
        .iter()
        .map(|item| {
            (
                item["title"].as_str().unwrap(),
                item["autocomplete"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        completions,
        [(
            "payments-worker",
            "status:error service:payments-worker -4h "
        )]
    );
    assert_eq!(items[1]["valid"], false);
}

#[test]
fn completions_are_per_env_and_substring() {
    let dir = tempfile::tempdir().unwrap();
    seed_services(dir.path(), "staging");

    let (items, _) = run(dir.path(), "env:staging service:api");
    let titles: Vec<&str> = items.as_array().unwrap()[1..]
        .iter()
        .map(|item| item["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["checkout-api", "search-api"]);

    let (items, _) = run(dir.path(), "service:api");
    assert_eq!(items.as_array().unwrap().len(), 1, "no production cache");
}

#[test]
fn a_complete_service_name_needs_no_completion() {
    let dir = tempfile::tempdir().unwrap();
    seed_services(dir.path(), "production");

    let (items, _) = run(dir.path(), "service:checkout-api");

    assert_eq!(items.as_array().unwrap().len(), 1);
}

#[test]
fn durations() {
    assert_eq!(duration(15 * MINUTE_MS), "15m");
    assert_eq!(duration(HOUR_MS), "1h");
    assert_eq!(duration(24 * HOUR_MS), "1d");
    assert_eq!(duration(90 * MINUTE_MS), "90m");
}

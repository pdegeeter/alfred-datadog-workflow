//! Tests for `ddm`: monitor parsing, pagination, env scope, tag filters,
//! ordering and items.

mod common;

use alfred_dd::filters;
use alfred_dd::frecency::Frecency;
use alfred_dd::monitors::{self, status_rank};
use alfred_dd::pup::Pup;
use alfred_dd::sources;
use common::{api, app, fixture, items_json, FakeRunner, NOW_MS, PUP};
use serde_json::Value;

fn titles(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap().to_owned())
        .collect()
}

fn runner() -> FakeRunner {
    FakeRunner::new()
        .with_success(PUP, &fixture("monitors_page0"))
        .with_success(PUP, &fixture("monitors_page1"))
}

#[test]
fn parses_a_page() {
    let page = monitors::parse(&fixture("monitors_page0")).unwrap();

    assert_eq!(page.page_count, 2);
    assert_eq!(page.monitors.len(), 3);
    let first = &page.monitors[0];
    assert_eq!(first.id, 101);
    assert_eq!(first.name, "Checkout API error rate");
    assert_eq!(first.status, "Alert");
    assert_eq!(first.kind, "query alert");
    assert_eq!(first.last_triggered_ms, Some(1_795_000_000_000));
    assert_eq!(page.monitors[2].last_triggered_ms, None);
}

#[test]
fn fetches_every_page() {
    let runner = runner();
    let pup = Pup::new(&runner, PUP);

    let all = sources::fetch_monitors(&pup).unwrap();

    assert_eq!(all.len(), 5);
    assert_eq!(
        runner.argvs(),
        vec![
            api("v1/monitor/search?per_page=1000&page=0"),
            api("v1/monitor/search?per_page=1000&page=1"),
        ]
    );
}

#[test]
fn states_order() {
    assert!(status_rank("Alert") < status_rank("Warn"));
    assert!(status_rank("Warn") < status_rank("No Data"));
    assert!(status_rank("No Data") < status_rank("OK"));
    assert!(status_rank("OK") < status_rank("Ignored"));
}

#[test]
fn lists_monitors_of_the_env_alerting_first() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items = items_json(filters::monitors(&app(&runner, dir.path()), "").items);

    assert_eq!(
        titles(&items),
        [
            "Checkout API error rate",
            "Payments worker latency",
            "Search API no traffic",
            "Billing cron failures",
            "Refresh monitors",
        ]
    );
}

#[test]
fn env_overrides_the_scope_and_untagged_monitors_always_count() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items = items_json(filters::monitors(&app(&runner, dir.path()), "env:staging").items);

    assert_eq!(
        titles(&items),
        [
            "Billing cron failures",
            "Checkout API staging errors",
            "Refresh monitors",
        ]
    );
}

#[test]
fn team_service_and_name_filters() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();
    let app = app(&runner, dir.path());

    assert_eq!(
        titles(&items_json(filters::monitors(&app, "team:payments").items)),
        [
            "Checkout API error rate",
            "Payments worker latency",
            "Refresh monitors"
        ]
    );
    assert_eq!(
        titles(&items_json(
            filters::monitors(&app, "service:search-api").items
        )),
        ["Search API no traffic", "Refresh monitors"]
    );
    assert_eq!(
        titles(&items_json(filters::monitors(&app, "latency").items)),
        ["Payments worker latency", "Refresh monitors"]
    );
}

#[test]
fn frecency_orders_monitors_within_a_state() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();
    let app = app(&runner, dir.path());
    let mut frecency = Frecency::default();
    frecency.record("monitor:105", NOW_MS);
    frecency
        .save(&Frecency::path(&app.config.data_dir))
        .unwrap();

    let items = items_json(filters::monitors(&app, "env:staging").items);

    assert_eq!(titles(&items)[0], "Billing cron failures");
    assert_eq!(
        items[0]["subtitle"],
        "OK · log alert · service:billing-cron, team:platform · triggered 2mo ago"
    );
}

#[test]
fn item_actions() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items =
        items_json(filters::monitors(&app(&runner, dir.path()), "checkout team:payments").items);
    let item = &items[0];

    assert_eq!(
        item["subtitle"],
        "Alert · query alert · service:checkout-api, team:payments · triggered 6d ago"
    );
    assert_eq!(
        item["arg"],
        "open monitor:101 https://app.datadoghq.eu/monitors/101"
    );
    assert_eq!(
        item["mods"]["cmd"]["arg"],
        "open-url https://app.datadoghq.eu/monitors/manage?q=checkout%20team%3Apayments"
    );
    assert_eq!(
        item["mods"]["alt"]["arg"],
        "copy https://app.datadoghq.eu/monitors/101"
    );
    assert!(item["icon"]["path"]
        .as_str()
        .unwrap()
        .ends_with("AlertStopIcon.icns"));
}

#[test]
fn no_match_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();
    let items = items_json(filters::monitors(&app(&runner, dir.path()), "kafka").items);
    assert_eq!(titles(&items)[0], "No matching monitor");

    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_failure(
        PUP,
        "`pup` exited with exit status: 173: Error: HTTP 429 https://api.datadoghq.eu/api/v1/monitor/search: Too many requests",
    );
    let items = items_json(filters::monitors(&app(&runner, dir.path()), "").items);
    assert_eq!(titles(&items)[0], "Datadog rate limit reached");
}

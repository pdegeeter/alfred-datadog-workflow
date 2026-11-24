//! Tests for `ddi`: incident parsing, commander resolution, ordering and
//! items.

mod common;

use alfred_dd::filters;
use alfred_dd::incidents::{self, severity_rank, Incident};
use common::{api, app, fixture, items_json, FakeRunner, PUP};
use serde_json::Value;

fn titles(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn parses_incidents_and_resolves_the_commander_from_the_facets() {
    let parsed = incidents::parse(&fixture("incidents")).unwrap();

    assert_eq!(parsed.len(), 3);
    assert_eq!(
        parsed[0],
        Incident {
            public_id: 12,
            title: "Checkout API returns 500".to_owned(),
            severity: Some("SEV-2".to_owned()),
            state: "active".to_owned(),
            commander: Some("Alice Martin".to_owned()),
            created_ms: Some(1_795_507_200_000),
        }
    );
    assert_eq!(parsed[2].commander, None);
}

#[test]
fn asks_for_active_and_stable_incidents() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(PUP, &fixture("incidents"));

    filters::incidents(&app(&runner, dir.path()), "");

    assert_eq!(runner.argvs(), vec![api(incidents::ENDPOINT)]);
    assert!(incidents::ENDPOINT.contains("state%3A%28active%20OR%20stable%29"));
}

#[test]
fn severity_order() {
    assert!(severity_rank(Some("SEV-1")) < severity_rank(Some("SEV-2")));
    assert!(severity_rank(Some("SEV-5")) < severity_rank(Some("UNKNOWN")));
    assert_eq!(severity_rank(None), severity_rank(Some("UNKNOWN")));
}

#[test]
fn most_severe_first_then_newest() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(PUP, &fixture("incidents"));

    let items = items_json(filters::incidents(&app(&runner, dir.path()), "").items);

    assert_eq!(
        titles(&items),
        [
            "#13 Payments delayed",
            "#12 Checkout API returns 500",
            "#11 Search slow for some customers",
            "Refresh incidents",
        ]
    );
    assert_eq!(items[0]["subtitle"], "SEV-1 · active · Bob Durand · 5h ago");
    assert_eq!(items[2]["subtitle"], "SEV-3 · stable · 21h ago");
    assert_eq!(
        items[0]["arg"],
        "open incident:13 https://app.datadoghq.eu/incidents/13"
    );
    assert_eq!(
        items[0]["mods"]["alt"]["arg"],
        "copy https://app.datadoghq.eu/incidents/13"
    );
}

#[test]
fn terms_filter_titles() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(PUP, &fixture("incidents"));

    let items = items_json(filters::incidents(&app(&runner, dir.path()), "checkout").items);

    assert_eq!(
        titles(&items),
        ["#12 Checkout API returns 500", "Refresh incidents"]
    );
}

#[test]
fn no_incident_is_said_plainly() {
    let dir = tempfile::tempdir().unwrap();
    let mut empty: Value = serde_json::from_str(&fixture("incidents")).unwrap();
    empty["data"]["attributes"]["incidents"] = Value::Array(Vec::new());
    let runner = FakeRunner::new().with_success(PUP, &empty.to_string());

    let items = items_json(filters::incidents(&app(&runner, dir.path()), "").items);

    assert_eq!(titles(&items), ["No active incident", "Refresh incidents"]);
    assert_eq!(items[0]["valid"], false);
}

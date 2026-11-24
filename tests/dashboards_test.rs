//! Tests for `ddd`: dashboards and notebooks parsing, pagination, ranking,
//! items and the cache round-trip.

mod common;

use alfred_dd::cache::{Entry, Key, Kind, Store};
use alfred_dd::dashboards::{self, Board, BoardKind};
use alfred_dd::filters;
use alfred_dd::frecency::Frecency;
use alfred_dd::notebooks;
use alfred_dd::pup::Pup;
use alfred_dd::sources;
use common::{api, app, fixture, items_json, FakeRunner, EXE, NOW_MS, PUP};
use serde_json::Value;

fn titles(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap().to_owned())
        .collect()
}

/// A runner answering the dashboards then the notebooks call.
fn runner() -> FakeRunner {
    FakeRunner::new()
        .with_success(PUP, &fixture("dashboards"))
        .with_success(PUP, &fixture("notebooks"))
}

#[test]
fn parses_dashboards() {
    let boards = dashboards::parse(&fixture("dashboards")).unwrap();

    assert_eq!(boards.len(), 4);
    assert_eq!(
        boards[0],
        Board {
            kind: BoardKind::Dashboard,
            id: "abc-def-ghi".to_owned(),
            title: "Checkout API overview".to_owned(),
            path: "/dashboard/abc-def-ghi/checkout-api-overview".to_owned(),
            author: "alice@example.com".to_owned(),
            detail: None,
            modified_ms: Some(1_795_168_800_000),
        }
    );
}

#[test]
fn deleted_dashboards_are_skipped() {
    let body = r#"{"dashboards":[
        {"id":"a","title":"Kept","url":"/dashboard/a/kept","deleted_at":null},
        {"id":"b","title":"Gone","url":"/dashboard/b/gone","deleted_at":"2026-01-01T00:00:00Z"}
    ]}"#;

    let boards = dashboards::parse(body).unwrap();

    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].title, "Kept");
}

#[test]
fn parses_notebooks() {
    let boards = notebooks::parse(&fixture("notebooks")).unwrap();

    assert_eq!(boards.len(), 3);
    assert_eq!(boards[0].kind, BoardKind::Notebook);
    assert_eq!(boards[0].id, "1001");
    assert_eq!(boards[0].title, "Checkout incident review");
    assert_eq!(boards[0].author, "Alice Martin");
    assert_eq!(boards[0].detail.as_deref(), Some("investigation"));
    assert_eq!(boards[2].detail, None);
}

#[test]
fn notebooks_are_fetched_page_by_page_until_a_short_page() {
    let template: Value = serde_json::from_str(&fixture("notebooks")).unwrap();
    let mut full_page = template.clone();
    full_page["data"] = Value::Array(
        (0..notebooks::PAGE_SIZE)
            .map(|index| {
                let mut notebook = template["data"][0].clone();
                notebook["id"] = Value::from(index as u64);
                notebook
            })
            .collect(),
    );
    let runner = FakeRunner::new()
        .with_success(PUP, &full_page.to_string())
        .with_success(PUP, &fixture("notebooks"));
    let pup = Pup::new(&runner, PUP);

    let boards = sources::fetch_notebooks(&pup).unwrap();

    assert_eq!(boards.len(), notebooks::PAGE_SIZE + 3);
    assert_eq!(
        runner.argvs(),
        vec![api(&notebooks::endpoint(0)), api(&notebooks::endpoint(100))]
    );
    assert!(notebooks::endpoint(0).contains("include_cells=false"));
}

#[test]
fn a_cold_cache_fetches_both_sources_and_lists_them_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let listing = filters::dashboards(&app(&runner, dir.path()), "");
    let items = items_json(listing.items);

    assert_eq!(
        titles(&items),
        [
            "Checkout API overview",
            "Checkout incident review",
            "Latence des paiements",
            "Équipe Paiements · latence",
            "Payments worker",
            "Capacity planning",
            "Platform SLOs",
            "Refresh dashboards & notebooks",
        ]
    );
    assert!(!listing.rerun);
    assert_eq!(
        runner.argvs(),
        vec![api(dashboards::ENDPOINT), api(&notebooks::endpoint(0))]
    );
}

#[test]
fn accents_and_word_prefixes_rank_matches() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items = items_json(filters::dashboards(&app(&runner, dir.path()), "equipe").items);

    assert_eq!(
        titles(&items),
        [
            "Équipe Paiements · latence",
            "Refresh dashboards & notebooks"
        ]
    );
}

#[test]
fn nb_and_db_restrict_the_kind() {
    let dir = tempfile::tempdir().unwrap();
    let notebooks_only = FakeRunner::new().with_success(PUP, &fixture("notebooks"));

    let items =
        items_json(filters::dashboards(&app(&notebooks_only, dir.path()), "nb:checkout").items);
    assert_eq!(
        titles(&items),
        ["Checkout incident review", "Refresh notebooks"]
    );
    assert_eq!(notebooks_only.argvs(), vec![api(&notebooks::endpoint(0))]);

    let dir = tempfile::tempdir().unwrap();
    let dashboards_only = FakeRunner::new().with_success(PUP, &fixture("dashboards"));
    let items =
        items_json(filters::dashboards(&app(&dashboards_only, dir.path()), "db: payments").items);
    assert_eq!(titles(&items), ["Payments worker", "Refresh dashboards"]);
}

#[test]
fn frecency_breaks_ties_and_orders_an_empty_query() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();
    let app = app(&runner, dir.path());
    let mut frecency = Frecency::default();
    frecency.record("dashboard:234-567-89a", NOW_MS - 60_000);
    frecency
        .save(&Frecency::path(&app.config.data_dir))
        .unwrap();

    let items = items_json(filters::dashboards(&app, "").items);
    assert_eq!(titles(&items)[0], "Platform SLOs");

    let items = items_json(filters::dashboards(&app, "p").items);
    assert_eq!(titles(&items)[0], "Platform SLOs");
}

#[test]
fn item_actions_and_modifiers() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items = items_json(filters::dashboards(&app(&runner, dir.path()), "checkout -4h").items);
    let dashboard = &items[0];

    assert_eq!(dashboard["title"], "Checkout API overview");
    assert_eq!(
        dashboard["subtitle"],
        "alice@example.com · Dashboard · modified 4d ago"
    );
    assert!(dashboard.get("uid").is_none());
    assert_eq!(
        dashboard["arg"],
        "open dashboard:abc-def-ghi https://app.datadoghq.eu/dashboard/abc-def-ghi/checkout-api-overview"
    );
    assert_eq!(
        dashboard["mods"]["cmd"]["arg"],
        format!(
            "open dashboard:abc-def-ghi https://app.datadoghq.eu/dashboard/abc-def-ghi/checkout-api-overview?from_ts={}&to_ts={NOW_MS}&live=true",
            NOW_MS - 4 * 3_600_000
        )
    );
    assert_eq!(
        dashboard["mods"]["alt"]["arg"],
        "copy https://app.datadoghq.eu/dashboard/abc-def-ghi/checkout-api-overview"
    );
    assert_eq!(
        dashboard["mods"]["shift"]["arg"],
        "open-url https://app.datadoghq.eu/dashboard/lists?q=checkout"
    );

    let notebook = &items[1];
    assert_eq!(notebook["title"], "Checkout incident review");
    assert_eq!(
        notebook["subtitle"],
        "Alice Martin · Notebook (investigation) · modified 6d ago"
    );
    assert_eq!(
        notebook["arg"],
        "open notebook:1001 https://app.datadoghq.eu/notebook/1001"
    );
}

#[test]
fn no_match_offers_the_dashboard_search() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items = items_json(filters::dashboards(&app(&runner, dir.path()), "kafka lag").items);

    assert_eq!(
        titles(&items)[0],
        "Search Datadog dashboards for “kafka lag”"
    );
    assert_eq!(
        items[0]["arg"],
        "open-url https://app.datadoghq.eu/dashboard/lists?q=kafka%20lag"
    );
}

#[test]
fn a_stale_cache_is_served_and_refreshed_in_the_background() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new();
    let app = app(&runner, dir.path());
    let store = Store::new(&app.config.cache_dir);
    let key = Key {
        site: "datadoghq.eu".to_owned(),
        scope: String::new(),
    };
    let stale = NOW_MS - 2 * 3_600_000;

    for (kind, fixture_name) in [
        (Kind::Dashboards, "dashboards"),
        (Kind::Notebooks, "notebooks"),
    ] {
        let items = match kind {
            Kind::Dashboards => dashboards::parse(&fixture(fixture_name)).unwrap(),
            _ => notebooks::parse(&fixture(fixture_name)).unwrap(),
        };
        store
            .write(
                kind,
                &Entry {
                    key: key.clone(),
                    fetched_ms: stale,
                    items,
                },
            )
            .unwrap();
    }

    let listing = filters::dashboards(&app, "");

    assert!(listing.rerun);
    assert_eq!(listing.items.len(), 8);
    let detached = runner.detached();
    assert_eq!(detached.len(), 2);
    assert_eq!(detached[0].program, EXE);
    assert_eq!(detached[0].args, ["refresh", "dashboards"]);
    assert_eq!(detached[1].args, ["refresh", "notebooks"]);
    let output = serde_json::to_value(listing.output()).unwrap();
    assert_eq!(output["rerun"], 0.5);
}

#[test]
fn a_pup_error_becomes_an_item_and_never_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_failure(
        PUP,
        "`pup` exited with exit status: 1: Error: HTTP 403 https://api.datadoghq.eu/api/v1/dashboard: {}",
    );

    let items = items_json(filters::dashboards(&app(&runner, dir.path()), "").items);

    assert_eq!(
        titles(&items),
        [
            "Datadog refused access to dashboards",
            "Datadog refused access to notebooks",
            "Refresh dashboards & notebooks",
        ]
    );
    assert_eq!(items[0]["valid"], false);
    assert_eq!(
        items[0]["subtitle"],
        "Your pup login needs the dashboards_read permission"
    );
}

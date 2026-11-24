//! Tests for `dds`: APM services, catalog definitions, merge, env and every
//! modifier.

mod common;

use alfred_dd::cache::{Key, Kind, Store};
use alfred_dd::filters;
use alfred_dd::pup::Pup;
use alfred_dd::services::{self, Definition, Service};
use alfred_dd::sources;
use common::{api, app, fixture, items_json, FakeRunner, EXE, NOW_MS, PUP};
use serde_json::Value;

const HOUR_MS: i64 = 3_600_000;

fn titles(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap().to_owned())
        .collect()
}

/// APM services then one short catalog page.
fn runner() -> FakeRunner {
    FakeRunner::new()
        .with_success(PUP, &fixture("apm_services"))
        .with_success(PUP, &fixture("service_definitions"))
}

#[test]
fn parses_apm_service_names() {
    assert_eq!(
        services::parse_apm(&fixture("apm_services")).unwrap(),
        [
            "checkout-api",
            "payments-worker",
            "search-api",
            "notifications-lambda"
        ]
    );
}

#[test]
fn parses_catalog_definitions() {
    let definitions = services::parse_definitions(&fixture("service_definitions")).unwrap();

    assert_eq!(definitions.len(), 3);
    assert_eq!(
        definitions[0],
        Definition {
            name: "checkout-api".to_owned(),
            team: Some("payments".to_owned()),
            languages: Vec::new(),
            kind: None,
        }
    );
}

#[test]
fn reads_languages_and_type_of_newer_schemas() {
    let mut body: Value = serde_json::from_str(&fixture("service_definitions")).unwrap();
    let schema = &mut body["data"][0]["attributes"]["schema"];
    schema["schema-version"] = Value::from("v2.2");
    schema["languages"] = serde_json::json!(["go"]);
    schema["type"] = Value::from("web");
    body["data"][1]["attributes"]["schema"] = serde_json::json!({
        "apiVersion": "v3",
        "kind": "service",
        "metadata": {"name": "ledger-api", "owner": "finance"},
        "spec": {"languages": ["java"], "type": "web"}
    });

    let definitions = services::parse_definitions(&body.to_string()).unwrap();

    assert_eq!(definitions[0].languages, ["go"]);
    assert_eq!(definitions[0].kind.as_deref(), Some("web"));
    assert_eq!(
        definitions[1],
        Definition {
            name: "ledger-api".to_owned(),
            team: Some("finance".to_owned()),
            languages: vec!["java".to_owned()],
            kind: Some("web".to_owned()),
        }
    );
}

#[test]
fn merge_is_the_union_by_name() {
    let merged = services::merge(
        vec!["checkout-api".to_owned(), "search-api".to_owned()],
        vec![
            Definition {
                name: "checkout-api".to_owned(),
                team: Some("payments".to_owned()),
                languages: vec!["go".to_owned()],
                kind: None,
            },
            Definition {
                name: "billing-cron".to_owned(),
                team: Some("platform".to_owned()),
                languages: Vec::new(),
                kind: None,
            },
        ],
    );

    assert_eq!(
        merged,
        [
            Service {
                name: "billing-cron".to_owned(),
                team: Some("platform".to_owned()),
                languages: Vec::new(),
                kind: None,
                traced: false,
            },
            Service {
                name: "checkout-api".to_owned(),
                team: Some("payments".to_owned()),
                languages: vec!["go".to_owned()],
                kind: None,
                traced: true,
            },
            Service {
                name: "search-api".to_owned(),
                team: None,
                languages: Vec::new(),
                kind: None,
                traced: true,
            },
        ]
    );
}

#[test]
fn fetch_asks_apm_for_the_last_day_in_the_env_then_pages_the_catalog() {
    let template: Value = serde_json::from_str(&fixture("service_definitions")).unwrap();
    let mut full_page = template.clone();
    full_page["data"] = Value::Array(
        (0..services::PAGE_SIZE)
            .map(|index| {
                let mut definition = template["data"][0].clone();
                definition["attributes"]["schema"]["dd-service"] =
                    Value::from(format!("service-{index}"));
                definition
            })
            .collect(),
    );
    let runner = FakeRunner::new()
        .with_success(PUP, &fixture("apm_services"))
        .with_success(PUP, &full_page.to_string())
        .with_success(PUP, &fixture("service_definitions"));
    let pup = Pup::new(&runner, PUP);

    let services = sources::fetch_services(&pup, "staging", NOW_MS).unwrap();

    assert_eq!(services.len(), services::PAGE_SIZE + 5);
    assert_eq!(
        runner.argvs(),
        vec![
            api(&format!(
                "v2/apm/services?filter[env]=staging&start={}&end={}",
                (NOW_MS - 24 * HOUR_MS) / 1000,
                NOW_MS / 1000
            )),
            api("v2/services/definitions?page[size]=100&page[number]=0"),
            api("v2/services/definitions?page[size]=100&page[number]=1"),
        ]
    );
}

#[test]
fn lists_traced_services_first_then_catalog_only_ones() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items = items_json(filters::services(&app(&runner, dir.path()), "").items);

    assert_eq!(
        titles(&items),
        [
            "checkout-api",
            "notifications-lambda",
            "payments-worker",
            "search-api",
            "billing-cron",
            "Refresh services (production)",
        ]
    );
    assert_eq!(items[0]["subtitle"], "payments · production");
    assert_eq!(items[4]["subtitle"], "platform · no traces in production");
}

#[test]
fn team_filter_and_terms() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items =
        items_json(filters::services(&app(&runner, dir.path()), "team:payments work").items);

    assert_eq!(
        titles(&items),
        ["payments-worker", "Refresh services (production)"]
    );
}

#[test]
fn every_modifier_respects_env_and_time_range() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items =
        items_json(filters::services(&app(&runner, dir.path()), "checkout env:staging -4h").items);
    let item = &items[0];
    let from = NOW_MS - 4 * HOUR_MS;

    assert_eq!(item["title"], "checkout-api");
    assert_eq!(
        item["arg"],
        format!("open service:checkout-api https://app.datadoghq.eu/apm/entity/service%3Acheckout-api?env=staging&start={from}&end={NOW_MS}&paused=false")
    );
    assert_eq!(
        item["mods"]["cmd"]["arg"],
        format!("open-url https://app.datadoghq.eu/logs?query=service%3Acheckout-api%20env%3Astaging&from_ts={from}&to_ts={NOW_MS}&live=true")
    );
    assert_eq!(
        item["mods"]["alt"]["arg"],
        format!("open-url https://app.datadoghq.eu/apm/traces?query=service%3Acheckout-api%20env%3Astaging&start={from}&end={NOW_MS}&paused=false")
    );
    assert_eq!(
        item["mods"]["ctrl"]["arg"],
        format!("open-url https://app.datadoghq.eu/error-tracking?query=service%3Acheckout-api%20env%3Astaging&from_ts={from}&to_ts={NOW_MS}&live=true")
    );
    assert_eq!(
        item["mods"]["shift"]["arg"],
        "open-url https://app.datadoghq.eu/monitors/manage?q=service%3Acheckout-api"
    );
    assert_eq!(item["mods"]["cmd+shift"]["arg"], "copy checkout-api");
    assert_eq!(
        items.as_array().unwrap().last().unwrap()["arg"],
        "refresh services:staging"
    );
}

#[test]
fn the_cache_is_per_env() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();
    let app = app(&runner, dir.path());

    filters::services(&app, "env:staging");

    let store = Store::new(&app.config.cache_dir);
    let staging = Key {
        site: "datadoghq.eu".to_owned(),
        scope: "staging".to_owned(),
    };
    let production = Key {
        site: "datadoghq.eu".to_owned(),
        scope: "production".to_owned(),
    };
    assert!(store.read::<Service>(Kind::Services, &staging).is_some());
    assert!(store.read::<Service>(Kind::Services, &production).is_none());
}

#[test]
fn a_stale_cache_refreshes_the_env_in_the_background() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();
    let app = app(&runner, dir.path());
    filters::services(&app, "env:staging");

    let later = alfred_dd::app::App::new(
        &runner,
        app.config.clone(),
        Some(PUP.to_owned()),
        EXE,
        NOW_MS + 2 * HOUR_MS,
    );
    let listing = filters::services(&later, "env:staging");

    assert!(listing.rerun);
    assert_eq!(
        runner.detached()[0].args,
        ["refresh", "services", "staging"]
    );
}

#[test]
fn no_match_and_errors_are_items() {
    let dir = tempfile::tempdir().unwrap();
    let runner = runner();

    let items = items_json(filters::services(&app(&runner, dir.path()), "kafka").items);
    assert_eq!(titles(&items)[0], "No matching service");
    assert_eq!(items[0]["valid"], false);

    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_failure(
        PUP,
        "`pup` exited with exit status: 1: Error: authentication required: run 'pup auth login'",
    );
    let items = items_json(filters::services(&app(&runner, dir.path()), "").items);
    assert_eq!(titles(&items)[0], "Run `pup auth login`");
}

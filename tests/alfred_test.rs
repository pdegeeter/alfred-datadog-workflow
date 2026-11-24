//! Item-level rules shared by every keyword.

mod common;

use alfred_dd::clipboard::PBPASTE;
use alfred_dd::filters::{self, Listing};
use common::{app, fixture, items_json, FakeRunner, PUP};

/// Every keyword's items, from the fixtures (and one error item).
fn every_listing(dir: &std::path::Path) -> Vec<Listing> {
    let runner = FakeRunner::new()
        .with_success(PBPASTE, "#12")
        .with_success(PUP, &fixture("dashboards"))
        .with_success(PUP, &fixture("notebooks"))
        .with_success(PUP, &fixture("apm_services"))
        .with_success(PUP, &fixture("service_definitions"))
        .with_success(PUP, &fixture("monitors_page0"))
        .with_success(PUP, &fixture("monitors_page1"))
        .with_success(PUP, &fixture("incidents"));
    let runner = Box::leak(Box::new(runner));
    let app = app(runner, dir);

    vec![
        filters::home(&app, ""),
        filters::dashboards(&app, ""),
        filters::services(&app, ""),
        filters::monitors(&app, ""),
        filters::logs(&app, "status:error"),
        filters::incidents(&app, ""),
        filters::monitors(&app, "nothing-matches-this"),
    ]
}

#[test]
fn no_item_sets_an_icon_or_a_uid() {
    let dir = tempfile::tempdir().unwrap();

    for listing in every_listing(dir.path()) {
        for item in items_json(listing.items).as_array().unwrap() {
            assert!(item.get("icon").is_none(), "{item}");
            assert!(item.get("uid").is_none(), "{item}");
        }
    }
}

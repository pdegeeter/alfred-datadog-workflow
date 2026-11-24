//! Tests for the `arg` protocol and the execution of every action.

mod common;

use alfred_dd::action::{self, Action, OPEN, OSASCRIPT};
use alfred_dd::cache::{Key, Kind, Store, Target};
use alfred_dd::dashboards::Board;
use alfred_dd::frecency::Frecency;
use common::{app, fixture, FakeRunner, NOW_MS, PUP};

fn all_actions() -> Vec<Action> {
    vec![
        Action::Open {
            key: "dashboard:abc-def-ghi".to_owned(),
            url: "https://app.datadoghq.eu/dashboard/abc-def-ghi/checkout?from_ts=1&to_ts=2&live=true"
                .to_owned(),
        },
        Action::Refresh {
            targets: vec![Target::new(Kind::Dashboards), Target::new(Kind::Notebooks)],
        },
        Action::OpenUrl {
            url: "https://app.datadoghq.eu/logs?query=service%3Acheckout-api".to_owned(),
        },
        Action::Copy {
            text: "checkout api with spaces".to_owned(),
        },
        Action::Search {
            query: "ddd ".to_owned(),
        },
    ]
}

#[test]
fn every_action_round_trips_through_its_arg() {
    for action in all_actions() {
        let arg = action.to_arg();

        assert_eq!(Action::parse(&arg).unwrap(), action, "arg: {arg:?}");
    }
}

#[test]
fn search_keeps_the_trailing_space_so_alfred_lands_after_the_keyword() {
    assert_eq!(
        Action::parse("search ddm ").unwrap(),
        Action::Search {
            query: "ddm ".to_owned()
        }
    );
}

#[test]
fn invalid_args_are_rejected() {
    for arg in ["", "nope x", "open-url", "copy ", "search"] {
        assert!(Action::parse(arg).is_err(), "{arg:?} should not parse");
    }
}

#[test]
fn open_url_runs_macos_open() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(OPEN, "");

    let message = action::execute(
        &app(&runner, dir.path()),
        &Action::OpenUrl {
            url: "https://app.datadoghq.eu/incidents/12".to_owned(),
        },
    )
    .unwrap();

    assert_eq!(message, None);
    assert_eq!(
        runner.argvs(),
        vec![vec!["https://app.datadoghq.eu/incidents/12".to_owned()]]
    );
}

#[test]
fn copy_passes_the_text_as_an_argv_item_and_notifies() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(OSASCRIPT, "");

    let message = action::execute(
        &app(&runner, dir.path()),
        &Action::Copy {
            text: "checkout-api".to_owned(),
        },
    )
    .unwrap();

    assert_eq!(message.as_deref(), Some("Copied checkout-api"));
    let call = &runner.calls()[0];
    assert_eq!(call.program, OSASCRIPT);
    assert_eq!(call.args[0], "-e");
    assert_eq!(call.args[2], "checkout-api");
}

#[test]
fn search_asks_alfred_to_show_the_query() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(OSASCRIPT, "");

    action::execute(
        &app(&runner, dir.path()),
        &Action::Search {
            query: "dds ".to_owned(),
        },
    )
    .unwrap();

    let call = &runner.calls()[0];
    assert!(call.args[1].contains("com.runningwithcrayons.Alfred"));
    assert_eq!(call.args[2], "dds ");
}

#[test]
fn refresh_rejects_unknown_sources() {
    assert!(Action::parse("refresh dashboards nope").is_err());
    assert!(Action::parse("refresh services:").is_err());
    assert!(Action::parse("refresh").is_err());
}

#[test]
fn refresh_carries_the_env_of_services() {
    let action = Action::Refresh {
        targets: vec![Target::services("staging"), Target::new(Kind::Monitors)],
    };

    assert_eq!(action.to_arg(), "refresh services:staging monitors");
    assert_eq!(Action::parse(&action.to_arg()).unwrap(), action);
}

#[test]
fn open_records_the_frecency_then_opens() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(OPEN, "");
    let app = app(&runner, dir.path());

    action::execute(
        &app,
        &Action::Open {
            key: "monitor:101".to_owned(),
            url: "https://app.datadoghq.eu/monitors/101".to_owned(),
        },
    )
    .unwrap();

    let frecency = Frecency::load(&Frecency::path(&app.config.data_dir));
    let visits = frecency.visits("monitor:101").unwrap();
    assert_eq!(visits.count, 1);
    assert_eq!(visits.last_ms, NOW_MS);
    assert_eq!(
        runner.argvs(),
        vec![vec!["https://app.datadoghq.eu/monitors/101".to_owned()]]
    );
}

#[test]
fn refresh_fetches_now_and_reports_counts() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new()
        .with_success(PUP, &fixture("dashboards"))
        .with_success(PUP, &fixture("notebooks"));
    let app = app(&runner, dir.path());

    let message = action::execute(
        &app,
        &Action::Refresh {
            targets: vec![Target::new(Kind::Dashboards), Target::new(Kind::Notebooks)],
        },
    )
    .unwrap();

    assert_eq!(
        message.as_deref(),
        Some("Refreshed 4 dashboards, 3 notebooks")
    );
    let store = Store::new(&app.config.cache_dir);
    let key = Key {
        site: "datadoghq.eu".to_owned(),
        scope: String::new(),
    };
    let entry = store.read::<Board>(Kind::Dashboards, &key).unwrap();
    assert_eq!(entry.fetched_ms, NOW_MS);
    assert_eq!(entry.items.len(), 4);
}

#[test]
fn refresh_surfaces_pup_errors() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new();
    let mut app = app(&runner, dir.path());
    app.pup_path = None;

    let err = action::execute(
        &app,
        &Action::Refresh {
            targets: vec![Target::new(Kind::Dashboards)],
        },
    )
    .unwrap_err();

    assert!(err.to_string().contains("brew install"), "{err}");
}

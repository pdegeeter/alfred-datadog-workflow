//! Tests for the `arg` protocol and the execution of every action.

mod common;

use alfred_dd::action::{self, Action, OPEN, OSASCRIPT};
use common::{app, FakeRunner};

fn all_actions() -> Vec<Action> {
    vec![
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

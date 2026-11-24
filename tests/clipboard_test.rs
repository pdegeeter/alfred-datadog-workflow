//! Tests for clipboard detection (table-driven) and the `dd` keyword.

mod common;

use alfred_dd::clipboard::{detect, Clip, PBPASTE};
use alfred_dd::filters;
use alfred_dd::urls::Urls;
use common::{app, items_json, FakeRunner};

const TRACE_128: &str = "69240a1e00000000a1b2c3d4e5f60718";

fn eu() -> Urls {
    Urls::new("datadoghq.eu")
}

#[test]
fn detection_table() {
    let cases: Vec<(&str, Vec<Clip>)> = vec![
        (TRACE_128, vec![Clip::Trace(TRACE_128.to_owned())]),
        (
            "69240A1E00000000A1B2C3D4E5F60718",
            vec![Clip::Trace(TRACE_128.to_owned())],
        ),
        (
            "  1234567890123456789\n",
            vec![
                Clip::Trace("1234567890123456789".to_owned()),
                Clip::Monitor(1_234_567_890_123_456_789),
            ],
        ),
        (
            "18446744073709551616",
            vec![Clip::Trace("18446744073709551616".to_owned())],
        ),
        (
            "101",
            vec![Clip::Trace("101".to_owned()), Clip::Monitor(101)],
        ),
        ("monitor 101", vec![Clip::Monitor(101)]),
        ("monitor:101", vec![Clip::Monitor(101)]),
        ("Monitor #101", vec![Clip::Monitor(101)]),
        ("monitor_id: 101", vec![Clip::Monitor(101)]),
        ("#12", vec![Clip::Incident(12)]),
        ("incident 12", vec![Clip::Incident(12)]),
        (
            "https://app.datadoghq.eu/monitors/101?from_ts=1",
            vec![Clip::Monitor(101)],
        ),
        (
            "https://app.datadoghq.com/incidents/12",
            vec![Clip::Incident(12)],
        ),
        (
            "https://app.datadoghq.com/logs?query=service%3Acheckout-api",
            vec![Clip::Url(
                "https://app.datadoghq.eu/logs?query=service%3Acheckout-api".to_owned(),
            )],
        ),
        (
            "https://app.datadoghq.eu/monitors/manage?q=x",
            vec![Clip::Url(
                "https://app.datadoghq.eu/monitors/manage?q=x".to_owned(),
            )],
        ),
    ];

    for (text, expected) in cases {
        assert_eq!(detect(text, &eu()), expected, "{text:?}");
    }
}

#[test]
fn nothing_else_is_detected() {
    for text in [
        "",
        "   ",
        "hello",
        "checkout api",
        "123456789012345678901",
        "69240a1e00000000a1b2c3d4e5f6071",
        "https://example.com/monitors/101",
        "#abc",
        "monitor checkout",
        "1 2",
    ] {
        assert_eq!(detect(text, &eu()), Vec::<Clip>::new(), "{text:?}");
    }

    assert_eq!(detect(&"1".repeat(3000), &eu()), Vec::<Clip>::new());
}

#[test]
fn dd_shows_the_clipboard_match_first_then_the_keywords() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(PBPASTE, &format!("{TRACE_128}\n"));

    let items = items_json(filters::home(&app(&runner, dir.path()), "").items);

    assert_eq!(items[0]["title"], format!("Trace {TRACE_128}"));
    assert_eq!(
        items[0]["arg"],
        format!("open-url https://app.datadoghq.eu/apm/trace/{TRACE_128}")
    );
    assert_eq!(
        items[0]["mods"]["alt"]["arg"],
        format!("copy https://app.datadoghq.eu/apm/trace/{TRACE_128}")
    );
    assert_eq!(items[1]["title"], "ddd — Dashboards & notebooks");
    assert_eq!(items[1]["arg"], "search ddd ");
    assert_eq!(items.as_array().unwrap().len(), 6);
    assert_eq!(runner.calls().len(), 1, "the clipboard is read once");
}

#[test]
fn an_ambiguous_number_offers_trace_and_monitor() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(PBPASTE, "4449274");

    let items = items_json(filters::home(&app(&runner, dir.path()), "").items);

    assert_eq!(items[0]["title"], "Trace 4449274");
    assert_eq!(items[1]["title"], "Monitor #4449274");
    assert_eq!(
        items[1]["arg"],
        "open monitor:4449274 https://app.datadoghq.eu/monitors/4449274"
    );
}

#[test]
fn an_id_typed_in_the_query_wins_without_reading_the_clipboard() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(PBPASTE, TRACE_128);

    let items = items_json(filters::home(&app(&runner, dir.path()), "#12").items);

    assert_eq!(items[0]["title"], "Incident #12");
    assert_eq!(
        items[0]["arg"],
        "open incident:12 https://app.datadoghq.eu/incidents/12"
    );
    assert!(runner.calls().is_empty());
}

#[test]
fn without_a_match_the_query_is_carried_to_the_keyword() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_success(PBPASTE, "some text");

    let items = items_json(filters::home(&app(&runner, dir.path()), "checkout").items);

    assert_eq!(items.as_array().unwrap().len(), 5);
    assert_eq!(items[0]["arg"], "search ddd checkout ");
    assert_eq!(items[4]["title"], "ddi — Incidents");
}

#[test]
fn an_unreadable_clipboard_just_shows_the_keywords() {
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new().with_failure(PBPASTE, "no pasteboard");

    let items = items_json(filters::home(&app(&runner, dir.path()), "").items);

    assert_eq!(items.as_array().unwrap().len(), 5);
}

//! Tests for timestamp parsing, relative times and percent-encoding.

use alfred_dd::percent::encode;
use alfred_dd::time::{ago, parse_rfc3339_ms};

const NOW_MS: i64 = 1_795_521_600_000;

#[test]
fn parses_the_timestamp_formats_datadog_returns() {
    assert_eq!(parse_rfc3339_ms("2026-11-24T12:00:00Z"), Some(NOW_MS));
    assert_eq!(
        parse_rfc3339_ms("2026-11-24T12:00:00.250000+00:00"),
        Some(NOW_MS + 250)
    );
    assert_eq!(parse_rfc3339_ms("2026-11-24T13:00:00+01:00"), Some(NOW_MS));
    assert_eq!(parse_rfc3339_ms("2026-11-24T11:30:00-00:30"), Some(NOW_MS));
    assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(
        parse_rfc3339_ms("2024-02-29T00:00:00Z"),
        Some(1_709_164_800_000)
    );
}

#[test]
fn rejects_garbage() {
    for value in ["", "yesterday", "2026-13-01T00:00:00Z", "2026-11-24"] {
        assert_eq!(parse_rfc3339_ms(value), None, "{value}");
    }
}

#[test]
fn ago_picks_the_largest_unit() {
    let cases = [
        (NOW_MS - 30_000, "just now"),
        (NOW_MS - 5 * 60_000, "5m ago"),
        (NOW_MS - 3 * 3_600_000, "3h ago"),
        (NOW_MS - 2 * 86_400_000, "2d ago"),
        (NOW_MS - 65 * 86_400_000, "2mo ago"),
        (NOW_MS - 800 * 86_400_000, "2y ago"),
        (NOW_MS + 60_000, "just now"),
    ];

    for (then, expected) in cases {
        assert_eq!(ago(then, NOW_MS), expected);
    }
}

#[test]
fn percent_encoding_keeps_unreserved_characters_only() {
    assert_eq!(
        encode("service:checkout-api env:prod"),
        "service%3Acheckout-api%20env%3Aprod"
    );
    assert_eq!(encode("a_b.c~d"), "a_b.c~d");
    assert_eq!(encode("équipe #1"), "%C3%A9quipe%20%231");
}

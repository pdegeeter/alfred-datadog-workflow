//! Tests for the shared query syntax.

use alfred_dd::query::{parse_range_ms, Query, DEFAULT_RANGE_MS};

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

#[test]
fn an_empty_query_has_the_default_range_and_nothing_else() {
    let query = Query::parse("   ");

    assert_eq!(query.terms, Vec::<String>::new());
    assert_eq!(query.env, None);
    assert_eq!(query.range_ms, DEFAULT_RANGE_MS);
    assert_eq!(DEFAULT_RANGE_MS, HOUR_MS);
}

#[test]
fn free_terms_are_kept_in_order() {
    assert_eq!(Query::parse("checkout  api").terms, ["checkout", "api"]);
}

#[test]
fn env_overrides_the_default_and_is_not_passed_through() {
    let query = Query::parse("env:staging checkout env:qa");

    assert_eq!(query.env.as_deref(), Some("qa"));
    assert_eq!(query.env_or("production"), "qa");
    assert_eq!(query.passthrough, ["checkout"]);
    assert_eq!(Query::parse("checkout").env_or("production"), "production");
}

#[test]
fn an_empty_env_is_a_filter_being_typed() {
    let query = Query::parse("env:");

    assert_eq!(query.env, None);
    assert_eq!(query.filters, [("env".to_owned(), String::new())]);
}

#[test]
fn time_suffixes() {
    let cases = [
        ("-15m", 15 * MINUTE_MS),
        ("-1h", HOUR_MS),
        ("-4h", 4 * HOUR_MS),
        ("-24h", 24 * HOUR_MS),
        ("-7d", 7 * DAY_MS),
    ];

    for (suffix, expected) in cases {
        assert_eq!(parse_range_ms(suffix), Some(expected), "{suffix}");
        assert_eq!(
            Query::parse(&format!("checkout {suffix}")).range_ms,
            expected
        );
    }
}

#[test]
fn a_time_suffix_only_counts_as_the_last_token() {
    let query = Query::parse("-4h checkout");

    assert_eq!(query.range_ms, DEFAULT_RANGE_MS);
    assert_eq!(query.terms, ["-4h", "checkout"]);
}

#[test]
fn invalid_suffixes_are_terms() {
    for token in ["-0h", "-h", "-4x", "4h", "-1.5h"] {
        assert_eq!(parse_range_ms(token), None, "{token}");
    }
}

#[test]
fn other_key_values_are_kept_verbatim_for_logs_and_traces() {
    let query = Query::parse(
        "service:checkout-api @http.status_code:500 -status:info timeout env:prod -15m",
    );

    assert_eq!(
        query.passthrough,
        [
            "service:checkout-api",
            "@http.status_code:500",
            "-status:info",
            "timeout"
        ]
    );
    assert_eq!(
        query.filters,
        [
            ("service".to_owned(), "checkout-api".to_owned()),
            ("@http.status_code".to_owned(), "500".to_owned()),
        ]
    );
    assert_eq!(query.terms, ["-status:info", "timeout"]);
    assert_eq!(query.filter_values("service"), ["checkout-api"]);
    assert!(query.has_filter("@http.status_code"));
}

#[test]
fn urls_and_times_are_not_filters() {
    let query = Query::parse("https://app.datadoghq.eu/x 10:30");

    assert!(query.filters.is_empty());
    assert_eq!(query.terms, ["https://app.datadoghq.eu/x", "10:30"]);
}

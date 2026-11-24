//! Tests for every Datadog URL builder, on the EU and US sites.

use alfred_dd::urls::{is_datadog_app_host, Urls, Window};

const NOW_MS: i64 = 1_795_521_600_000;
const HOUR_MS: i64 = 3_600_000;

fn last_hour() -> Window {
    Window::last(HOUR_MS, NOW_MS)
}

#[test]
fn window_ends_now_and_is_live() {
    let window = Window::last(4 * HOUR_MS, NOW_MS);

    assert_eq!(window.from_ms, NOW_MS - 4 * HOUR_MS);
    assert_eq!(window.to_ms, NOW_MS);
    assert!(window.live);
}

#[test]
fn every_url_uses_the_configured_site() {
    for (site, base) in [
        ("datadoghq.eu", "https://app.datadoghq.eu"),
        ("datadoghq.com", "https://app.datadoghq.com"),
        ("us3.datadoghq.com", "https://app.us3.datadoghq.com"),
    ] {
        let urls = Urls::new(site);

        assert_eq!(urls.base(), base);
        assert_eq!(urls.incident(12), format!("{base}/incidents/12"));
        assert_eq!(urls.monitor(101), format!("{base}/monitors/101"));
        assert_eq!(urls.notebook(1001), format!("{base}/notebook/1001"));
    }
}

#[test]
fn dashboard_urls() {
    let urls = Urls::new("datadoghq.eu");

    assert_eq!(
        urls.dashboard("/dashboard/abc-def-ghi/checkout-api-overview"),
        "https://app.datadoghq.eu/dashboard/abc-def-ghi/checkout-api-overview"
    );
    assert_eq!(
        urls.dashboard_at("/dashboard/abc-def-ghi/checkout-api-overview", last_hour()),
        format!(
            "https://app.datadoghq.eu/dashboard/abc-def-ghi/checkout-api-overview?from_ts={}&to_ts={NOW_MS}&live=true",
            NOW_MS - HOUR_MS
        )
    );
    assert_eq!(
        urls.dashboard_search("checkout api"),
        "https://app.datadoghq.eu/dashboard/lists?q=checkout%20api"
    );
}

#[test]
fn notebook_urls() {
    let urls = Urls::new("datadoghq.com");

    assert_eq!(
        urls.notebook_at(1001, last_hour()),
        format!(
            "https://app.datadoghq.com/notebook/1001?from_ts={}&to_ts={NOW_MS}&live=true",
            NOW_MS - HOUR_MS
        )
    );
    assert_eq!(
        urls.notebook_search("capacity"),
        "https://app.datadoghq.com/notebook/list?q=capacity"
    );
}

#[test]
fn service_page() {
    assert_eq!(
        Urls::new("datadoghq.eu").service("checkout-api", "production", last_hour()),
        format!(
            "https://app.datadoghq.eu/apm/entity/service%3Acheckout-api?env=production&start={}&end={NOW_MS}&paused=false",
            NOW_MS - HOUR_MS
        )
    );
}

#[test]
fn logs_explorer_takes_from_ts_to_ts_and_live() {
    assert_eq!(
        Urls::new("datadoghq.eu").logs("service:checkout-api env:production", last_hour()),
        format!(
            "https://app.datadoghq.eu/logs?query=service%3Acheckout-api%20env%3Aproduction&from_ts={}&to_ts={NOW_MS}&live=true",
            NOW_MS - HOUR_MS
        )
    );
}

#[test]
fn traces_explorer_takes_start_and_end() {
    assert_eq!(
        Urls::new("datadoghq.com").traces("service:checkout-api env:production", last_hour()),
        format!(
            "https://app.datadoghq.com/apm/traces?query=service%3Acheckout-api%20env%3Aproduction&start={}&end={NOW_MS}&paused=false",
            NOW_MS - HOUR_MS
        )
    );
}

#[test]
fn a_range_that_does_not_end_now_is_paused() {
    let window = Window {
        from_ms: 1,
        to_ms: 2,
        live: false,
    };

    assert_eq!(
        Urls::new("datadoghq.eu").traces("x", window),
        "https://app.datadoghq.eu/apm/traces?query=x&start=1&end=2&paused=true"
    );
    assert_eq!(
        Urls::new("datadoghq.eu").logs("x", window),
        "https://app.datadoghq.eu/logs?query=x&from_ts=1&to_ts=2&live=false"
    );
}

#[test]
fn error_tracking() {
    assert_eq!(
        Urls::new("datadoghq.eu").error_tracking("service:checkout-api env:production", last_hour()),
        format!(
            "https://app.datadoghq.eu/error-tracking?query=service%3Acheckout-api%20env%3Aproduction&from_ts={}&to_ts={NOW_MS}&live=true",
            NOW_MS - HOUR_MS
        )
    );
}

#[test]
fn monitor_search() {
    assert_eq!(
        Urls::new("datadoghq.eu").monitor_search("service:checkout-api"),
        "https://app.datadoghq.eu/monitors/manage?q=service%3Acheckout-api"
    );
}

#[test]
fn a_blank_search_opens_the_plain_list() {
    let urls = Urls::new("datadoghq.eu");

    assert_eq!(
        urls.dashboard_search(" "),
        "https://app.datadoghq.eu/dashboard/lists"
    );
    assert_eq!(
        urls.notebook_search(""),
        "https://app.datadoghq.eu/notebook/list"
    );
    assert_eq!(
        urls.monitor_search(""),
        "https://app.datadoghq.eu/monitors/manage"
    );
}

#[test]
fn trace_by_id() {
    let urls = Urls::new("datadoghq.eu");

    assert_eq!(
        urls.trace("69240a1e00000000a1b2c3d4e5f60718"),
        "https://app.datadoghq.eu/apm/trace/69240a1e00000000a1b2c3d4e5f60718"
    );
    assert_eq!(
        urls.trace("1234567890123456789"),
        "https://app.datadoghq.eu/apm/trace/1234567890123456789"
    );
}

#[test]
fn datadog_urls_are_moved_to_the_configured_site() {
    let urls = Urls::new("datadoghq.eu");

    assert_eq!(
        urls.on_this_site("https://app.datadoghq.com/logs?query=a#x")
            .as_deref(),
        Some("https://app.datadoghq.eu/logs?query=a#x")
    );
    assert_eq!(
        urls.on_this_site("https://app.datadoghq.eu").as_deref(),
        Some("https://app.datadoghq.eu")
    );
    assert_eq!(urls.on_this_site("https://example.com/logs"), None);
    assert_eq!(urls.on_this_site("app.datadoghq.com/logs"), None);
}

#[test]
fn datadog_hosts() {
    for host in [
        "app.datadoghq.com",
        "app.datadoghq.eu",
        "us3.datadoghq.com",
        "myorg.datadoghq.eu",
        "app.ddog-gov.com",
    ] {
        assert!(is_datadog_app_host(host), "{host}");
    }

    for host in ["example.com", "datadoghq.com.evil.io", "notdatadoghq.com"] {
        assert!(!is_datadog_app_host(host), "{host}");
    }
}

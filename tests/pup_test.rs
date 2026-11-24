//! Tests for pup invocation, error classification and refresh-and-retry.

mod common;

use alfred_dd::pup::{classify, locate_in, Failure, Pup, PupError, CANDIDATES};
use common::FakeRunner;

const PUP: &str = "/opt/homebrew/bin/pup";

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

/// What `SystemRunner` reports when pup exits non-zero.
fn exited(code: u8, stderr: &str) -> String {
    format!("`{PUP}` exited with exit status: {code}: {stderr}")
}

#[test]
fn get_runs_pup_api_without_the_envelope() {
    let runner = FakeRunner::new().with_success(PUP, "{\"dashboards\":[]}");
    let pup = Pup::new(&runner, PUP);

    assert_eq!(pup.get("v1/dashboard").unwrap(), "{\"dashboards\":[]}");
    assert_eq!(
        runner.argvs(),
        vec![argv(&["--no-envelope", "api", "v1/dashboard"])]
    );
}

#[test]
fn a_401_refreshes_the_session_once_and_retries() {
    let runner = FakeRunner::new()
        .with_failure(PUP, &exited(1, "Error: HTTP 401 https://api.datadoghq.eu/api/v1/dashboard: {\"errors\":[\"Unauthorized\"]}"))
        .with_success(PUP, "")
        .with_success(PUP, "{\"dashboards\":[]}");
    let pup = Pup::new(&runner, PUP);

    assert_eq!(pup.get("v1/dashboard").unwrap(), "{\"dashboards\":[]}");
    assert_eq!(
        runner.argvs(),
        vec![
            argv(&["--no-envelope", "api", "v1/dashboard"]),
            argv(&["auth", "refresh"]),
            argv(&["--no-envelope", "api", "v1/dashboard"]),
        ]
    );
}

#[test]
fn a_401_after_the_refresh_means_logging_in_again() {
    let unauthorized = exited(
        1,
        "Error: HTTP 401 https://api.datadoghq.eu/api/v1/monitor/search: {}",
    );
    let runner = FakeRunner::new()
        .with_failure(PUP, &unauthorized)
        .with_success(PUP, "")
        .with_failure(PUP, &unauthorized);
    let pup = Pup::new(&runner, PUP);

    assert_eq!(pup.get("v1/monitor/search"), Err(PupError::NotLoggedIn));
    assert_eq!(runner.calls().len(), 3);
}

#[test]
fn a_failed_refresh_means_logging_in_again_without_retrying() {
    let runner = FakeRunner::new()
        .with_failure(
            PUP,
            &exited(
                1,
                "Error: authentication expired. Run 'pup auth refresh' to renew your session",
            ),
        )
        .with_failure(
            PUP,
            &exited(
                1,
                "Error: no refresh token available — run 'pup auth login' to re-authenticate",
            ),
        );
    let pup = Pup::new(&runner, PUP);

    assert_eq!(pup.get("v1/dashboard"), Err(PupError::NotLoggedIn));
    assert_eq!(
        runner.argvs(),
        vec![
            argv(&["--no-envelope", "api", "v1/dashboard"]),
            argv(&["auth", "refresh"]),
        ]
    );
}

#[test]
fn errors_other_than_401_are_not_retried() {
    let runner = FakeRunner::new().with_failure(
        PUP,
        &exited(1, "Error: HTTP 403 https://api.datadoghq.eu/api/v1/dashboard: {\"errors\":[\"Forbidden\"]}"),
    );
    let pup = Pup::new(&runner, PUP);

    assert_eq!(pup.get("v1/dashboard"), Err(PupError::Forbidden));
    assert_eq!(runner.calls().len(), 1);
}

#[test]
fn a_missing_binary_is_reported_as_missing() {
    let runner = FakeRunner::new();
    let pup = Pup::new(&runner, "/nowhere/pup");

    assert_eq!(pup.get("v1/dashboard"), Err(PupError::Missing));
}

#[test]
fn classification_table() {
    let cases = [
        ("failed to spawn `/opt/homebrew/bin/pup`: No such file or directory", Failure::Missing),
        (
            "Error: authentication required: set DD_ACCESS_TOKEN for bearer auth, run 'pup auth login' for OAuth2",
            Failure::NotLoggedIn,
        ),
        ("Error: no tokens found for site datadoghq.eu — run 'pup auth login' first", Failure::NotLoggedIn),
        ("Error: authentication expired. Run 'pup auth refresh'", Failure::Unauthorized),
        ("Error: HTTP 401 https://api.datadoghq.eu/api/v1/dashboard: {}", Failure::Unauthorized),
        ("Error: failed to list dashboards: ResponseError(ResponseContent { status: 401, … })", Failure::Unauthorized),
        ("Error: HTTP 403 https://api.datadoghq.eu/api/v1/dashboard: {\"errors\":[\"Forbidden\"]}", Failure::Forbidden),
        ("Error: HTTP 429 https://api.datadoghq.eu/api/v1/monitor/search: Too many requests", Failure::RateLimited),
        ("exited with exit status: 173: Hint: rate limited — wait and retry", Failure::RateLimited),
        (
            "`pup` exited with exit status: 1: Error: HTTP 500 https://api.datadoghq.eu/api/v1/dashboard: {\"errors\":[\"boom\"]}",
            Failure::Other("HTTP 500 https://api.datadoghq.eu/api/v1/dashboard: {\"errors\":[\"boom\"]}".to_owned()),
        ),
    ];

    for (message, expected) in cases {
        assert_eq!(classify(message), expected, "{message}");
    }
}

#[test]
fn an_id_containing_401_is_not_an_authentication_error() {
    assert!(matches!(
        classify("Error: HTTP 404 https://api.datadoghq.eu/api/v1/monitor/40123: Not found"),
        Failure::Other(_)
    ));
}

#[test]
fn errors_explain_what_to_do() {
    assert!(PupError::Missing
        .to_string()
        .contains("brew install datadog-labs/pack/pup"));
    assert!(PupError::NotLoggedIn.to_string().contains("pup auth login"));
    assert!(PupError::RateLimited.to_string().contains("retry"));
}

#[test]
fn locate_prefers_the_first_existing_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let homebrew = dir.path().join("homebrew-pup");
    let usr_local = dir.path().join("usr-local-pup");
    std::fs::write(&usr_local, "").unwrap();

    let homebrew = homebrew.to_str().unwrap();
    let usr_local = usr_local.to_str().unwrap();

    assert_eq!(
        locate_in(&[homebrew, usr_local]).as_deref(),
        Some(usr_local)
    );

    std::fs::write(homebrew, "").unwrap();
    assert_eq!(locate_in(&[homebrew, usr_local]).as_deref(), Some(homebrew));

    assert_eq!(locate_in(&["/nowhere/pup"]), None);
}

#[test]
fn candidates_are_homebrew_on_apple_silicon_then_intel() {
    assert_eq!(CANDIDATES, ["/opt/homebrew/bin/pup", "/usr/local/bin/pup"]);
}

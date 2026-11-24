//! Tests for frecency recording, scoring and persistence.

mod common;

use alfred_dd::frecency::Frecency;
use common::NOW_MS;

const DAY_MS: i64 = 24 * 3_600_000;

#[test]
fn record_counts_opens_and_keeps_the_last_one() {
    let mut frecency = Frecency::default();

    frecency.record("dashboard:abc-def-ghi", NOW_MS - DAY_MS);
    frecency.record("dashboard:abc-def-ghi", NOW_MS);

    let visits = frecency.visits("dashboard:abc-def-ghi").unwrap();
    assert_eq!(visits.count, 2);
    assert_eq!(visits.last_ms, NOW_MS);
}

#[test]
fn score_is_zero_for_an_object_never_opened() {
    assert_eq!(Frecency::default().score("service:x", NOW_MS), 0.0);
}

#[test]
fn score_halves_every_week() {
    let mut frecency = Frecency::default();
    frecency.record("service:checkout-api", NOW_MS - 7 * DAY_MS);
    frecency.record("service:checkout-api", NOW_MS - 7 * DAY_MS);

    let score = frecency.score("service:checkout-api", NOW_MS);

    assert!((score - 1.0).abs() < 1e-9, "{score}");
}

#[test]
fn recent_opens_outrank_older_frequent_ones() {
    let mut frecency = Frecency::default();
    for _ in 0..4 {
        frecency.record("monitor:1", NOW_MS - 30 * DAY_MS);
    }
    frecency.record("monitor:2", NOW_MS - DAY_MS);

    assert!(frecency.score("monitor:2", NOW_MS) > frecency.score("monitor:1", NOW_MS));
}

#[test]
fn save_then_load_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = Frecency::path(&dir.path().join("data"));
    let mut frecency = Frecency::default();
    frecency.record("incident:12", NOW_MS);

    frecency.save(&path).unwrap();

    assert_eq!(Frecency::load(&path), frecency);
}

#[test]
fn a_missing_or_corrupted_file_is_an_empty_frecency() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frecency.json");

    assert_eq!(Frecency::load(&path), Frecency::default());

    std::fs::write(&path, "[1, 2").unwrap();
    assert_eq!(Frecency::load(&path), Frecency::default());
}

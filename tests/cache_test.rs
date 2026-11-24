//! Tests for the stale-while-revalidate cache.

mod common;

use std::cell::Cell;
use std::time::Duration;

use alfred_dd::cache::{freshness, Entry, Freshness, Key, Kind, Store};
use alfred_dd::pup::PupError;
use common::NOW_MS;

const MINUTE_MS: i64 = 60_000;

fn key() -> Key {
    Key {
        site: "datadoghq.eu".to_owned(),
        scope: String::new(),
    }
}

fn seed(store: &Store, kind: Kind, fetched_ms: i64, items: &[&str]) {
    store
        .write(
            kind,
            &Entry {
                key: key(),
                fetched_ms,
                items: items.iter().map(|item| item.to_string()).collect(),
            },
        )
        .unwrap();
}

#[test]
fn ttl_per_kind() {
    assert_eq!(Kind::Dashboards.ttl(), Duration::from_secs(3600));
    assert_eq!(Kind::Notebooks.ttl(), Duration::from_secs(3600));
    assert_eq!(Kind::Services.ttl(), Duration::from_secs(3600));
    assert_eq!(Kind::Monitors.ttl(), Duration::from_secs(120));
    assert_eq!(Kind::Incidents.ttl(), Duration::from_secs(60));
}

#[test]
fn kinds_round_trip_through_their_name() {
    for kind in Kind::ALL {
        assert_eq!(Kind::parse(kind.name()), Some(kind));
    }

    assert_eq!(Kind::parse("nope"), None);
}

#[test]
fn freshness_is_relative_to_the_injected_clock() {
    let ttl = Duration::from_secs(60);

    assert_eq!(freshness(NOW_MS - 59_999, NOW_MS, ttl), Freshness::Fresh);
    assert_eq!(freshness(NOW_MS - 60_000, NOW_MS, ttl), Freshness::Stale);
}

#[test]
fn a_fresh_cache_is_served_without_fetching_or_refreshing() {
    for kind in Kind::ALL {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        seed(&store, kind, NOW_MS - 10_000, &["a", "b"]);

        let refreshed = Cell::new(false);
        let loaded = store.load::<String, _, _>(
            kind,
            &key(),
            NOW_MS,
            || panic!("must not fetch"),
            || refreshed.set(true),
        );

        assert_eq!(loaded.items, ["a", "b"], "{kind}");
        assert!(!loaded.rerun, "{kind}");
        assert!(!refreshed.get(), "{kind}");
    }
}

#[test]
fn a_stale_cache_is_served_and_refreshed_in_the_background() {
    for kind in Kind::ALL {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let stale = NOW_MS - kind.ttl().as_millis() as i64 - MINUTE_MS;
        seed(&store, kind, stale, &["old"]);

        let refreshed = Cell::new(0);
        let loaded = store.load::<String, _, _>(
            kind,
            &key(),
            NOW_MS,
            || panic!("must not fetch synchronously"),
            || refreshed.set(refreshed.get() + 1),
        );

        assert_eq!(loaded.items, ["old"], "{kind}");
        assert_eq!(loaded.fetched_ms, Some(stale));
        assert!(loaded.rerun, "{kind}");
        assert_eq!(refreshed.get(), 1, "{kind}");
    }
}

#[test]
fn a_running_refresh_is_not_started_twice_but_alfred_keeps_rerunning() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    seed(&store, Kind::Monitors, NOW_MS - 10 * MINUTE_MS, &["old"]);
    let _lock = store.lock(Kind::Monitors).unwrap();

    let loaded = store.load::<String, _, _>(
        Kind::Monitors,
        &key(),
        NOW_MS,
        || panic!("must not fetch"),
        || panic!("must not start a second refresh"),
    );

    assert!(loaded.rerun);
}

#[test]
fn a_recently_failed_refresh_is_not_retried_on_every_rerun() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    seed(&store, Kind::Incidents, NOW_MS - 10 * MINUTE_MS, &["old"]);

    let result = store.refresh::<String, _>(Kind::Incidents, &key(), NOW_MS, || {
        Err(PupError::RateLimited)
    });
    assert_eq!(result, Err(PupError::RateLimited));

    let loaded = store.load::<String, _, _>(
        Kind::Incidents,
        &key(),
        NOW_MS,
        || panic!("must not fetch"),
        || panic!("must not restart a refresh that just failed"),
    );

    assert_eq!(loaded.items, ["old"]);
    assert!(!loaded.rerun);
}

#[test]
fn a_missing_cache_is_fetched_synchronously_and_written() {
    for kind in Kind::ALL {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());

        let loaded = store.load::<String, _, _>(
            kind,
            &key(),
            NOW_MS,
            || Ok(vec!["fresh".to_owned()]),
            || panic!("must not refresh in the background"),
        );

        assert_eq!(loaded.items, ["fresh"], "{kind}");
        assert!(!loaded.rerun);
        assert_eq!(
            store.read::<String>(kind, &key()).unwrap().fetched_ms,
            NOW_MS
        );
    }
}

#[test]
fn a_corrupted_cache_is_treated_as_missing() {
    for kind in Kind::ALL {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        std::fs::write(store.path(kind), "{not json").unwrap();

        let loaded = store.load::<String, _, _>(
            kind,
            &key(),
            NOW_MS,
            || Ok(vec!["fresh".to_owned()]),
            || panic!("must not refresh in the background"),
        );

        assert_eq!(loaded.items, ["fresh"], "{kind}");
    }
}

#[test]
fn a_cache_written_for_another_site_or_scope_is_treated_as_missing() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    seed(&store, Kind::Services, NOW_MS, &["eu"]);

    let other_site = Key {
        site: "datadoghq.com".to_owned(),
        scope: String::new(),
    };
    let other_env = Key {
        site: "datadoghq.eu".to_owned(),
        scope: "staging".to_owned(),
    };

    assert!(store.read::<String>(Kind::Services, &other_site).is_none());
    assert!(store.read::<String>(Kind::Services, &other_env).is_none());
    assert!(store.read::<String>(Kind::Services, &key()).is_some());
}

#[test]
fn a_failed_synchronous_fetch_returns_the_error_and_no_items() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());

    let loaded = store.load::<String, _, _>(
        Kind::Dashboards,
        &key(),
        NOW_MS,
        || Err(PupError::NotLoggedIn),
        || panic!("must not refresh"),
    );

    assert!(loaded.items.is_empty());
    assert_eq!(loaded.error, Some(PupError::NotLoggedIn));
    assert!(store.read::<String>(Kind::Dashboards, &key()).is_none());
}

#[test]
fn refresh_replaces_the_cache_and_releases_its_lock() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    seed(&store, Kind::Dashboards, NOW_MS - 2 * 3_600_000, &["old"]);

    let count = store
        .refresh(Kind::Dashboards, &key(), NOW_MS, || {
            Ok(vec!["a".to_owned(), "b".to_owned()])
        })
        .unwrap();

    assert_eq!(count, Some(2));
    let entry = store.read::<String>(Kind::Dashboards, &key()).unwrap();
    assert_eq!(entry.items, ["a", "b"]);
    assert_eq!(entry.fetched_ms, NOW_MS);
    assert!(!store.is_locked(Kind::Dashboards));
}

#[test]
fn refresh_is_skipped_while_another_one_holds_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let _lock = store.lock(Kind::Notebooks).unwrap();

    let count = store
        .refresh::<String, _>(Kind::Notebooks, &key(), NOW_MS, || panic!("must not fetch"))
        .unwrap();

    assert_eq!(count, None);
}

#[test]
fn an_abandoned_lock_is_taken_over() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let path = store.lock_path(Kind::Monitors);
    std::fs::write(&path, "1").unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(120);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old)
        .unwrap();

    assert!(!store.is_locked(Kind::Monitors));
    assert!(store.lock(Kind::Monitors).is_some());
}

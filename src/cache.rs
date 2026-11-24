//! Stale-while-revalidate cache: one JSON file per source under
//! `$alfred_workflow_cache`.
//!
//! A script filter serves the cached list at once. When it is older than the
//! source's TTL, it also starts `alfred-dd refresh <kind>` in the background
//! (unless one is already running: lock file) and asks Alfred to rerun it.
//! Without a cache it fetches synchronously. A file that does not parse, or
//! that was fetched for another site or scope (e.g. another env), counts as
//! missing.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::pup::PupError;

/// A lock older than this is considered abandoned (crashed refresh).
pub const LOCK_TIMEOUT: Duration = Duration::from_secs(60);

/// Every cached source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Dashboards,
    Notebooks,
    Services,
    Monitors,
    Incidents,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Dashboards,
        Kind::Notebooks,
        Kind::Services,
        Kind::Monitors,
        Kind::Incidents,
    ];

    /// Name used in file names, the `refresh` verb and messages.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Dashboards => "dashboards",
            Kind::Notebooks => "notebooks",
            Kind::Services => "services",
            Kind::Monitors => "monitors",
            Kind::Incidents => "incidents",
        }
    }

    pub fn parse(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// How long a fetched list is served without revalidation.
    pub fn ttl(self) -> Duration {
        match self {
            Kind::Dashboards | Kind::Notebooks | Kind::Services => Duration::from_secs(60 * 60),
            Kind::Monitors => Duration::from_secs(2 * 60),
            Kind::Incidents => Duration::from_secs(60),
        }
    }

    /// The Datadog permission (OAuth scope) its endpoints need.
    pub fn scope(self) -> &'static str {
        match self {
            Kind::Dashboards => "dashboards_read",
            Kind::Notebooks => "notebooks_read",
            Kind::Services => "apm_read, apm_service_catalog_read",
            Kind::Monitors => "monitors_read",
            Kind::Incidents => "incident_read",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What a cached list was fetched for. A cache written for another key is
/// ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub site: String,
    /// Extra discriminant: the env for services, empty otherwise.
    pub scope: String,
}

/// A cached list, as stored on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry<T> {
    pub key: Key,
    pub fetched_ms: i64,
    pub items: Vec<T>,
}

/// Age of a cached list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    Stale,
}

/// Whether a list fetched at `fetched_ms` is still fresh at `now_ms`.
pub fn freshness(fetched_ms: i64, now_ms: i64, ttl: Duration) -> Freshness {
    if now_ms - fetched_ms < ttl.as_millis() as i64 {
        return Freshness::Fresh;
    }

    Freshness::Stale
}

/// What a script filter gets back from [`Store::load`].
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded<T> {
    pub items: Vec<T>,
    /// When the items were fetched (`None` if nothing could be fetched).
    pub fetched_ms: Option<i64>,
    /// A background refresh is (or should be) running: ask Alfred to rerun.
    pub rerun: bool,
    /// The synchronous fetch failed (only without a usable cache).
    pub error: Option<PupError>,
}

/// The cache directory.
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    pub fn path(&self, kind: Kind) -> PathBuf {
        self.dir.join(format!("{}.json", kind.name()))
    }

    pub fn lock_path(&self, kind: Kind) -> PathBuf {
        self.dir.join(format!("{}.lock", kind.name()))
    }

    /// The cached list for `key`, if readable and written for that key.
    pub fn read<T: DeserializeOwned>(&self, kind: Kind, key: &Key) -> Option<Entry<T>> {
        let text = fs::read_to_string(self.path(kind)).ok()?;
        let entry: Entry<T> = serde_json::from_str(&text).ok()?;

        if &entry.key != key {
            return None;
        }

        Some(entry)
    }

    /// Replace the cached list atomically (temporary file + rename).
    pub fn write<T: Serialize>(&self, kind: Kind, entry: &Entry<T>) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)?;

        let path = self.path(kind);
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        let json = serde_json::to_vec(entry).map_err(std::io::Error::other)?;

        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)
    }

    /// Marker left by a failed refresh, so a stale list does not respawn a
    /// failing refresh on every rerun.
    pub fn failure_path(&self, kind: Kind) -> PathBuf {
        self.dir.join(format!("{}.failed", kind.name()))
    }

    /// Whether a refresh of `kind` is running (a lock younger than
    /// [`LOCK_TIMEOUT`] exists).
    pub fn is_locked(&self, kind: Kind) -> bool {
        lock_is_live(&self.lock_path(kind))
    }

    /// Whether the last refresh of `kind` failed less than [`LOCK_TIMEOUT`]
    /// ago.
    pub fn failed_recently(&self, kind: Kind) -> bool {
        lock_is_live(&self.failure_path(kind))
    }

    /// Take the refresh lock of `kind`; `None` if another refresh holds it.
    pub fn lock(&self, kind: Kind) -> Option<Lock> {
        fs::create_dir_all(&self.dir).ok()?;

        let path = self.lock_path(kind);

        if path.exists() && !lock_is_live(&path) {
            let _ = fs::remove_file(&path);
        }

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .ok()?;
        let _ = write!(file, "{}", std::process::id());

        Some(Lock { path })
    }

    /// Serve `kind` for `key`, stale-while-revalidate.
    ///
    /// - fresh cache → served;
    /// - stale cache → served, `start_refresh` is called unless a refresh is
    ///   already running, and `rerun` is set;
    /// - no usable cache → `fetch` is called synchronously and its result
    ///   cached.
    pub fn load<T, F, R>(
        &self,
        kind: Kind,
        key: &Key,
        now_ms: i64,
        fetch: F,
        start_refresh: R,
    ) -> Loaded<T>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Result<Vec<T>, PupError>,
        R: FnOnce(),
    {
        if let Some(entry) = self.read::<T>(kind, key) {
            let rerun = match freshness(entry.fetched_ms, now_ms, kind.ttl()) {
                Freshness::Fresh => self.is_locked(kind),
                Freshness::Stale if self.is_locked(kind) => true,
                Freshness::Stale if self.failed_recently(kind) => false,
                Freshness::Stale => {
                    start_refresh();

                    true
                }
            };

            return Loaded {
                items: entry.items,
                fetched_ms: Some(entry.fetched_ms),
                rerun,
                error: None,
            };
        }

        match fetch() {
            Ok(items) => {
                let entry = Entry {
                    key: key.clone(),
                    fetched_ms: now_ms,
                    items,
                };
                let _ = self.write(kind, &entry);

                Loaded {
                    items: entry.items,
                    fetched_ms: Some(now_ms),
                    rerun: false,
                    error: None,
                }
            }
            Err(error) => Loaded {
                items: Vec::new(),
                fetched_ms: None,
                rerun: false,
                error: Some(error),
            },
        }
    }

    /// Fetch `kind` and replace its cache, under the refresh lock. Returns the
    /// number of items, or `None` when another refresh holds the lock.
    pub fn refresh<T, F>(
        &self,
        kind: Kind,
        key: &Key,
        now_ms: i64,
        fetch: F,
    ) -> Result<Option<usize>, PupError>
    where
        T: Serialize,
        F: FnOnce() -> Result<Vec<T>, PupError>,
    {
        let Some(_lock) = self.lock(kind) else {
            return Ok(None);
        };

        let items = match fetch() {
            Ok(items) => items,
            Err(error) => {
                let _ = fs::write(self.failure_path(kind), error.to_string());

                return Err(error);
            }
        };
        let _ = fs::remove_file(self.failure_path(kind));
        let count = items.len();
        let entry = Entry {
            key: key.clone(),
            fetched_ms: now_ms,
            items,
        };

        self.write(kind, &entry)
            .map_err(|err| PupError::Other(format!("cannot write the {kind} cache: {err}")))?;

        Ok(Some(count))
    }
}

fn lock_is_live(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };

    let age = metadata
        .modified()
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .unwrap_or_default();

    age < LOCK_TIMEOUT
}

/// A held refresh lock, released on drop.
pub struct Lock {
    path: PathBuf,
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

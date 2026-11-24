//! How often and how recently each Datadog object was opened from Alfred.
//!
//! Stored as `frecency.json` in `$alfred_workflow_data`, keyed by
//! `<kind>:<id>` (`dashboard:abc-def-ghi`, `service:checkout-api`…), and
//! updated by the `open` action. Used as the primary order on an empty query
//! and as a tie-breaker everywhere else.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A visit's weight halves every week.
pub const HALF_LIFE_MS: f64 = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;

/// File name inside the data directory.
pub const FILE_NAME: &str = "frecency.json";

/// Opens of one object.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Visits {
    pub count: u32,
    pub last_ms: i64,
}

/// Every object's visits.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Frecency {
    entries: BTreeMap<String, Visits>,
}

impl Frecency {
    /// Where the frecency of a data directory lives.
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join(FILE_NAME)
    }

    /// Read from `path`; a missing or corrupted file is an empty frecency.
    pub fn load(path: &Path) -> Frecency {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Write to `path` atomically.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        let json = serde_json::to_vec(self).map_err(std::io::Error::other)?;

        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)
    }

    /// Record one open of `key` at `now_ms`.
    pub fn record(&mut self, key: &str, now_ms: i64) {
        let visits = self.entries.entry(key.to_owned()).or_insert(Visits {
            count: 0,
            last_ms: now_ms,
        });

        visits.count += 1;
        visits.last_ms = now_ms;
    }

    pub fn visits(&self, key: &str) -> Option<Visits> {
        self.entries.get(key).copied()
    }

    /// `count`, decayed by the time since the last open (half-life one
    /// week). 0 for an object never opened.
    pub fn score(&self, key: &str, now_ms: i64) -> f64 {
        let Some(visits) = self.visits(key) else {
            return 0.0;
        };

        let age = (now_ms - visits.last_ms).max(0) as f64;

        f64::from(visits.count) * 0.5_f64.powf(age / HALF_LIFE_MS)
    }
}

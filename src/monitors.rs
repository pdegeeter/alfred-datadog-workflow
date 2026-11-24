//! Monitors (`GET /api/v1/monitor/search`, paged with `page` / `per_page`).

use serde::{Deserialize, Serialize};

/// Monitors fetched per page (the API accepts up to 1000).
pub const PAGE_SIZE: usize = 1000;

/// Endpoint of page `page` (0-based).
pub fn endpoint(page: usize) -> String {
    format!("v1/monitor/search?per_page={PAGE_SIZE}&page={page}")
}

/// A monitor as listed by `ddm`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Monitor {
    pub id: u64,
    pub name: String,
    /// `Alert`, `Warn`, `No Data`, `OK`…
    pub status: String,
    /// `query alert`, `log alert`, `metric alert`…
    pub kind: String,
    pub tags: Vec<String>,
    pub last_triggered_ms: Option<i64>,
}

impl Monitor {
    /// Frecency key: `monitor:<id>`.
    pub fn frecency_key(&self) -> String {
        format!("monitor:{}", self.id)
    }

    /// Values of its `key:value` tags for `key`.
    pub fn tag_values<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a str> {
        self.tags.iter().filter_map(move |tag| {
            tag.split_once(':')
                .filter(|(name, _)| *name == key)
                .map(|(_, value)| value)
        })
    }

    /// Whether the monitor applies to `env`: tagged `env:<env>`, or with no
    /// `env:` tag at all.
    pub fn in_env(&self, env: &str) -> bool {
        let mut envs = self.tag_values("env").peekable();

        envs.peek().is_none() || envs.any(|value| value == env)
    }
}

/// Order of states: alerting first.
pub fn status_rank(status: &str) -> u8 {
    match status {
        "Alert" => 0,
        "Warn" => 1,
        "No Data" => 2,
        "OK" => 3,
        _ => 4,
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    monitors: Vec<Raw>,
    #[serde(default)]
    metadata: Option<Metadata>,
}

#[derive(Deserialize)]
struct Metadata {
    #[serde(default)]
    page_count: usize,
}

#[derive(Deserialize)]
struct Raw {
    id: u64,
    name: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    last_triggered_ts: Option<i64>,
}

/// One parsed page and the total number of pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub monitors: Vec<Monitor>,
    pub page_count: usize,
}

/// Parse one page of `GET /api/v1/monitor/search`.
pub fn parse(body: &str) -> serde_json::Result<Page> {
    let response: Response = serde_json::from_str(body)?;

    Ok(Page {
        page_count: response.metadata.map_or(1, |metadata| metadata.page_count),
        monitors: response
            .monitors
            .into_iter()
            .map(|raw| Monitor {
                id: raw.id,
                name: raw.name,
                status: raw.status.unwrap_or_default(),
                kind: raw.kind.unwrap_or_default(),
                tags: raw.tags,
                last_triggered_ms: raw.last_triggered_ts.map(|seconds| seconds * 1000),
            })
            .collect(),
    })
}

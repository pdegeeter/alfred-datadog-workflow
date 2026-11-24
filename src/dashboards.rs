//! Dashboards (`GET /api/v1/dashboard`) and the [`Board`] type shared with
//! notebooks.

use serde::{Deserialize, Serialize};

use crate::time::parse_rfc3339_ms;
use crate::urls::{Urls, Window};

/// Every dashboard in one call (the endpoint is not paginated).
pub const ENDPOINT: &str = "v1/dashboard";

/// Dashboard or notebook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoardKind {
    Dashboard,
    Notebook,
}

/// A dashboard or a notebook, as listed by `ddd`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub kind: BoardKind,
    pub id: String,
    pub title: String,
    /// App path of a dashboard (`/dashboard/<id>/<slug>`); empty for notebooks.
    pub path: String,
    /// Author handle (dashboards) or name (notebooks).
    pub author: String,
    /// Notebook type (`investigation`, `report`…), if any.
    pub detail: Option<String>,
    pub modified_ms: Option<i64>,
}

impl Board {
    /// Frecency key: `dashboard:<id>` / `notebook:<id>`.
    pub fn frecency_key(&self) -> String {
        match self.kind {
            BoardKind::Dashboard => format!("dashboard:{}", self.id),
            BoardKind::Notebook => format!("notebook:{}", self.id),
        }
    }

    pub fn url(&self, urls: &Urls) -> String {
        match self.kind {
            BoardKind::Dashboard => urls.dashboard(&self.path),
            BoardKind::Notebook => urls.notebook(self.notebook_id()),
        }
    }

    pub fn url_at(&self, urls: &Urls, window: Window) -> String {
        match self.kind {
            BoardKind::Dashboard => urls.dashboard_at(&self.path, window),
            BoardKind::Notebook => urls.notebook_at(self.notebook_id(), window),
        }
    }

    fn notebook_id(&self) -> u64 {
        self.id.parse().unwrap_or_default()
    }
}

#[derive(Deserialize)]
struct Response {
    dashboards: Vec<Dashboard>,
}

#[derive(Deserialize)]
struct Dashboard {
    id: String,
    title: String,
    url: String,
    #[serde(default)]
    author_handle: Option<String>,
    #[serde(default)]
    modified_at: Option<String>,
    #[serde(default)]
    deleted_at: Option<String>,
}

/// Parse `GET /api/v1/dashboard`; deleted dashboards are skipped.
pub fn parse(body: &str) -> serde_json::Result<Vec<Board>> {
    let response: Response = serde_json::from_str(body)?;

    Ok(response
        .dashboards
        .into_iter()
        .filter(|dashboard| dashboard.deleted_at.is_none())
        .map(|dashboard| Board {
            kind: BoardKind::Dashboard,
            id: dashboard.id,
            title: dashboard.title,
            path: dashboard.url,
            author: dashboard.author_handle.unwrap_or_default(),
            detail: None,
            modified_ms: dashboard.modified_at.as_deref().and_then(parse_rfc3339_ms),
        })
        .collect())
}

//! Notebooks (`GET /api/v1/notebooks`, paginated with `start` / `count`).

use serde::Deserialize;

use crate::dashboards::{Board, BoardKind};
use crate::time::parse_rfc3339_ms;

/// Notebooks fetched per page.
pub const PAGE_SIZE: usize = 100;

/// Endpoint of the page starting at `start`, most recently modified first,
/// without cells.
pub fn endpoint(start: usize) -> String {
    format!(
        "v1/notebooks?count={PAGE_SIZE}&start={start}&sort_field=modified&sort_dir=desc&include_cells=false"
    )
}

#[derive(Deserialize)]
struct Response {
    data: Vec<Notebook>,
}

#[derive(Deserialize)]
struct Notebook {
    id: u64,
    attributes: Attributes,
}

#[derive(Deserialize)]
struct Attributes {
    name: String,
    #[serde(default)]
    modified: Option<String>,
    #[serde(default)]
    author: Option<Author>,
    #[serde(default)]
    metadata: Option<Metadata>,
}

#[derive(Deserialize)]
struct Author {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    handle: Option<String>,
}

#[derive(Deserialize)]
struct Metadata {
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

/// Parse one page of `GET /api/v1/notebooks`.
pub fn parse(body: &str) -> serde_json::Result<Vec<Board>> {
    let response: Response = serde_json::from_str(body)?;

    Ok(response
        .data
        .into_iter()
        .map(|notebook| {
            let attributes = notebook.attributes;
            let author = attributes
                .author
                .and_then(|author| {
                    author
                        .name
                        .filter(|name| !name.is_empty())
                        .or(author.handle)
                })
                .unwrap_or_default();

            Board {
                kind: BoardKind::Notebook,
                id: notebook.id.to_string(),
                title: attributes.name,
                path: String::new(),
                author,
                detail: attributes.metadata.and_then(|metadata| metadata.kind),
                modified_ms: attributes.modified.as_deref().and_then(parse_rfc3339_ms),
            }
        })
        .collect())
}

//! APM services: the names seen by APM in an env
//! (`GET /api/v2/apm/services`), enriched with the Software Catalog
//! definitions (`GET /api/v2/services/definitions`: team, languages, type).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::percent::encode;

/// Definitions fetched per catalog page.
pub const PAGE_SIZE: usize = 100;

/// How far back APM looks for services: a day, so quiet services still show.
pub const APM_LOOKBACK_MS: i64 = 24 * 60 * 60 * 1000;

/// A service as listed by `dds`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub team: Option<String>,
    pub languages: Vec<String>,
    /// Catalog type (`web`, `db`, `function`…).
    pub kind: Option<String>,
    /// Seen by APM in the env (not only declared in the catalog).
    pub traced: bool,
}

impl Service {
    /// Frecency key: `service:<name>`.
    pub fn frecency_key(&self) -> String {
        format!("service:{}", self.name)
    }
}

/// Endpoint of the services APM saw in `env` during the day before `now_ms`.
pub fn apm_endpoint(env: &str, now_ms: i64) -> String {
    let end = now_ms / 1000;
    let start = (now_ms - APM_LOOKBACK_MS) / 1000;

    format!(
        "v2/apm/services?filter[env]={}&start={start}&end={end}",
        encode(env)
    )
}

/// Endpoint of catalog page `page` (0-based).
pub fn definitions_endpoint(page: usize) -> String {
    format!("v2/services/definitions?page[size]={PAGE_SIZE}&page[number]={page}")
}

#[derive(Deserialize)]
struct ApmResponse {
    data: ApmData,
}

#[derive(Deserialize)]
struct ApmData {
    attributes: ApmAttributes,
}

#[derive(Deserialize)]
struct ApmAttributes {
    #[serde(default)]
    services: Vec<String>,
}

/// Parse `GET /api/v2/apm/services`: the service names.
pub fn parse_apm(body: &str) -> serde_json::Result<Vec<String>> {
    let response: ApmResponse = serde_json::from_str(body)?;

    Ok(response.data.attributes.services)
}

/// What the catalog says about a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub name: String,
    pub team: Option<String>,
    pub languages: Vec<String>,
    pub kind: Option<String>,
}

#[derive(Deserialize)]
struct DefinitionsResponse {
    #[serde(default)]
    data: Vec<DefinitionData>,
}

#[derive(Deserialize)]
struct DefinitionData {
    attributes: DefinitionAttributes,
}

#[derive(Deserialize)]
struct DefinitionAttributes {
    #[serde(default)]
    schema: serde_json::Value,
}

/// Parse one page of `GET /api/v2/services/definitions`. Schema v2.x
/// (`dd-service`, `team`, `languages`, `type`) and v3 (`metadata.name`,
/// `metadata.owner`, `spec.languages`, `spec.type`) are both read; entries
/// without a name are skipped.
pub fn parse_definitions(body: &str) -> serde_json::Result<Vec<Definition>> {
    let response: DefinitionsResponse = serde_json::from_str(body)?;

    Ok(response
        .data
        .into_iter()
        .filter_map(|definition| read_schema(&definition.attributes.schema))
        .collect())
}

fn text(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn texts(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|values| values.iter().filter_map(text).collect())
        .unwrap_or_default()
}

fn read_schema(schema: &serde_json::Value) -> Option<Definition> {
    if let Some(name) = text(&schema["dd-service"]) {
        return Some(Definition {
            name,
            team: text(&schema["team"]),
            languages: texts(&schema["languages"]),
            kind: text(&schema["type"]),
        });
    }

    let name = text(&schema["metadata"]["name"])?;

    Some(Definition {
        name,
        team: text(&schema["metadata"]["owner"]),
        languages: texts(&schema["spec"]["languages"]),
        kind: text(&schema["spec"]["type"]),
    })
}

/// Every service APM saw, plus every catalog service, by name.
pub fn merge(traced: Vec<String>, definitions: Vec<Definition>) -> Vec<Service> {
    let mut services: BTreeMap<String, Service> = traced
        .into_iter()
        .map(|name| {
            let service = Service {
                name: name.clone(),
                team: None,
                languages: Vec::new(),
                kind: None,
                traced: true,
            };

            (name, service)
        })
        .collect();

    for definition in definitions {
        let service = services
            .entry(definition.name.clone())
            .or_insert_with(|| Service {
                name: definition.name.clone(),
                team: None,
                languages: Vec::new(),
                kind: None,
                traced: false,
            });

        service.team = definition.team;
        service.languages = definition.languages;
        service.kind = definition.kind;
    }

    services.into_values().collect()
}

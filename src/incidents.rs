//! Incidents that are not resolved (`GET /api/v2/incidents/search`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::time::parse_rfc3339_ms;

/// Active and stable incidents, up to 100.
pub const ENDPOINT: &str =
    "v2/incidents/search?query=state%3A%28active%20OR%20stable%29&page[size]=100";

/// An incident as listed by `ddi`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Incident {
    /// The `#12` shown in the app and used in its URL.
    pub public_id: u64,
    pub title: String,
    /// `SEV-1`…`SEV-5`, `UNKNOWN`.
    pub severity: Option<String>,
    pub state: String,
    pub commander: Option<String>,
    pub created_ms: Option<i64>,
}

impl Incident {
    /// Frecency key: `incident:<public_id>`.
    pub fn frecency_key(&self) -> String {
        format!("incident:{}", self.public_id)
    }
}

/// Order of severities: SEV-1 first, unknown last.
pub fn severity_rank(severity: Option<&str>) -> u32 {
    severity
        .and_then(|severity| severity.strip_prefix("SEV-"))
        .and_then(|level| level.parse().ok())
        .unwrap_or(u32::MAX)
}

#[derive(Deserialize)]
struct Response {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    attributes: Attributes,
}

#[derive(Deserialize)]
struct Attributes {
    #[serde(default)]
    incidents: Vec<Wrapper>,
    #[serde(default)]
    facets: Facets,
}

#[derive(Deserialize, Default)]
struct Facets {
    #[serde(default)]
    commander: Vec<Person>,
}

#[derive(Deserialize)]
struct Person {
    uuid: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    handle: Option<String>,
}

#[derive(Deserialize)]
struct Wrapper {
    data: Raw,
}

#[derive(Deserialize)]
struct Raw {
    attributes: RawAttributes,
    #[serde(default)]
    relationships: Option<Relationships>,
}

#[derive(Deserialize)]
struct RawAttributes {
    public_id: u64,
    title: String,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    created: Option<String>,
}

#[derive(Deserialize)]
struct Relationships {
    #[serde(default)]
    commander_user: Option<Relation>,
}

#[derive(Deserialize)]
struct Relation {
    #[serde(default)]
    data: Option<Reference>,
}

#[derive(Deserialize)]
struct Reference {
    id: String,
}

/// Parse `GET /api/v2/incidents/search`. The commander's name comes from the
/// `commander` facet of the same response (the users are not `included`).
pub fn parse(body: &str) -> serde_json::Result<Vec<Incident>> {
    let response: Response = serde_json::from_str(body)?;
    let attributes = response.data.attributes;
    let people: HashMap<String, String> = attributes
        .facets
        .commander
        .into_iter()
        .filter_map(|person| {
            let name = person
                .name
                .filter(|name| !name.is_empty())
                .or(person.handle)?;

            Some((person.uuid, name))
        })
        .collect();

    Ok(attributes
        .incidents
        .into_iter()
        .map(|wrapper| {
            let raw = wrapper.data;
            let commander = raw
                .relationships
                .and_then(|relationships| relationships.commander_user)
                .and_then(|relation| relation.data)
                .and_then(|reference| people.get(&reference.id).cloned());

            Incident {
                public_id: raw.attributes.public_id,
                title: raw.attributes.title,
                severity: raw.attributes.severity,
                state: raw.attributes.state.unwrap_or_default(),
                commander,
                created_ms: raw.attributes.created.as_deref().and_then(parse_rfc3339_ms),
            }
        })
        .collect())
}

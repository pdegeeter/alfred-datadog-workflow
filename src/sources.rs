//! Fetching each source through pup, and serving it through the cache.

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::app::App;
use crate::cache::{Key, Kind, Loaded, Store, Target};
use crate::dashboards::{self, Board};
use crate::incidents::{self, Incident};
use crate::monitors::{self, Monitor};
use crate::notebooks;
use crate::pup::{Pup, PupError};
use crate::services::{self, Service};

/// Safety net against a pagination loop that never ends.
pub const MAX_PAGES: usize = 50;

/// The env a target is about: its own, else `DD_ENV`.
pub fn env_of<'a>(app: &'a App, target: &'a Target) -> &'a str {
    target.env.as_deref().unwrap_or(&app.config.env)
}

/// What a cached list of `target` is valid for: the site, plus the env for
/// services (the APM service list depends on it).
pub fn key(app: &App, target: &Target) -> Key {
    let scope = match target.kind {
        Kind::Services => env_of(app, target).to_owned(),
        _ => String::new(),
    };

    Key {
        site: app.config.site.clone(),
        scope,
    }
}

/// `GET endpoint` parsed with `parse`; a body that does not parse is an error
/// naming the source.
pub fn get<T>(
    pup: &Pup,
    kind: Kind,
    endpoint: &str,
    parse: impl Fn(&str) -> serde_json::Result<T>,
) -> Result<T, PupError> {
    let body = pup.get(endpoint)?;

    parse(&body).map_err(|err| PupError::Other(format!("unexpected {kind} response: {err}")))
}

/// Pages of `page_size` items, from page 0 until a short page.
fn paged<T>(
    page_size: usize,
    mut fetch_page: impl FnMut(usize, usize) -> Result<Vec<T>, PupError>,
) -> Result<Vec<T>, PupError> {
    let mut all = Vec::new();

    for page in 0..MAX_PAGES {
        let items = fetch_page(page, all.len())?;
        let full = items.len() == page_size;
        all.extend(items);

        if !full {
            break;
        }
    }

    Ok(all)
}

pub fn fetch_dashboards(pup: &Pup) -> Result<Vec<Board>, PupError> {
    get(
        pup,
        Kind::Dashboards,
        dashboards::ENDPOINT,
        dashboards::parse,
    )
}

/// Every notebook, page by page.
pub fn fetch_notebooks(pup: &Pup) -> Result<Vec<Board>, PupError> {
    paged(notebooks::PAGE_SIZE, |_, start| {
        get(
            pup,
            Kind::Notebooks,
            &notebooks::endpoint(start),
            notebooks::parse,
        )
    })
}

/// Services APM saw in `env`, enriched with every catalog definition.
pub fn fetch_services(pup: &Pup, env: &str, now_ms: i64) -> Result<Vec<Service>, PupError> {
    let traced = get(
        pup,
        Kind::Services,
        &services::apm_endpoint(env, now_ms),
        services::parse_apm,
    )?;
    let definitions = paged(services::PAGE_SIZE, |page, _| {
        get(
            pup,
            Kind::Services,
            &services::definitions_endpoint(page),
            services::parse_definitions,
        )
    })?;

    Ok(services::merge(traced, definitions))
}

/// Every monitor, page by page until `page_count`.
pub fn fetch_monitors(pup: &Pup) -> Result<Vec<Monitor>, PupError> {
    let mut all = Vec::new();

    for page in 0..MAX_PAGES {
        let parsed = get(
            pup,
            Kind::Monitors,
            &monitors::endpoint(page),
            monitors::parse,
        )?;
        let last = parsed.monitors.is_empty() || page + 1 >= parsed.page_count;
        all.extend(parsed.monitors);

        if last {
            break;
        }
    }

    Ok(all)
}

/// Active and stable incidents.
pub fn fetch_incidents(pup: &Pup) -> Result<Vec<Incident>, PupError> {
    get(pup, Kind::Incidents, incidents::ENDPOINT, incidents::parse)
}

/// Serve `target` from the cache, fetching with `fetch` when there is none
/// and starting `alfred-dd refresh <kind> [env]` in the background when it is
/// stale.
pub fn load<T>(
    app: &App,
    target: &Target,
    fetch: impl FnOnce(&Pup) -> Result<Vec<T>, PupError>,
) -> Loaded<T>
where
    T: Serialize + DeserializeOwned,
{
    let store = Store::new(&app.config.cache_dir);
    let mut args = vec!["refresh", target.kind.name()];

    if target.kind == Kind::Services {
        args.push(env_of(app, target));
    }

    store.load(
        target.kind,
        &key(app, target),
        app.now_ms,
        || fetch(&app.pup()?),
        || {
            let _ = app.runner.spawn_detached(&app.exe, &args);
        },
    )
}

pub fn load_dashboards(app: &App) -> Loaded<Board> {
    load(app, &Target::new(Kind::Dashboards), fetch_dashboards)
}

pub fn load_notebooks(app: &App) -> Loaded<Board> {
    load(app, &Target::new(Kind::Notebooks), fetch_notebooks)
}

pub fn load_monitors(app: &App) -> Loaded<Monitor> {
    load(app, &Target::new(Kind::Monitors), fetch_monitors)
}

pub fn load_incidents(app: &App) -> Loaded<Incident> {
    load(app, &Target::new(Kind::Incidents), fetch_incidents)
}

pub fn load_services(app: &App, env: &str) -> Loaded<Service> {
    load(app, &Target::services(env), |pup| {
        fetch_services(pup, env, app.now_ms)
    })
}

/// Fetch `target` now and replace its cache. `Ok(None)` when another refresh
/// is already running.
pub fn refresh(app: &App, target: &Target) -> Result<Option<usize>, PupError> {
    let store = Store::new(&app.config.cache_dir);
    let key = key(app, target);
    let pup = app.pup()?;
    let kind = target.kind;

    match kind {
        Kind::Dashboards => store.refresh(kind, &key, app.now_ms, || fetch_dashboards(&pup)),
        Kind::Notebooks => store.refresh(kind, &key, app.now_ms, || fetch_notebooks(&pup)),
        Kind::Services => store.refresh(kind, &key, app.now_ms, || {
            fetch_services(&pup, env_of(app, target), app.now_ms)
        }),
        Kind::Monitors => store.refresh(kind, &key, app.now_ms, || fetch_monitors(&pup)),
        Kind::Incidents => store.refresh(kind, &key, app.now_ms, || fetch_incidents(&pup)),
    }
}

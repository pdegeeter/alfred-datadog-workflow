//! Fetching each source through pup, and serving it through the cache.

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::app::App;
use crate::cache::{Key, Kind, Loaded, Store};
use crate::dashboards::{self, Board};
use crate::notebooks;
use crate::pup::{Pup, PupError};

/// Safety net against a pagination loop that never ends.
pub const MAX_PAGES: usize = 50;

/// What a cached list of `kind` is valid for: the site, plus the env for
/// services (the APM service list depends on it).
pub fn key(app: &App, kind: Kind) -> Key {
    let scope = match kind {
        Kind::Services => app.config.env.clone(),
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
    let mut all = Vec::new();

    for _ in 0..MAX_PAGES {
        let page = get(
            pup,
            Kind::Notebooks,
            &notebooks::endpoint(all.len()),
            notebooks::parse,
        )?;
        let full = page.len() == notebooks::PAGE_SIZE;
        all.extend(page);

        if !full {
            break;
        }
    }

    Ok(all)
}

/// Serve `kind` from the cache, fetching with `fetch` when there is none and
/// starting `alfred-dd refresh <kind>` in the background when it is stale.
pub fn load<T>(
    app: &App,
    kind: Kind,
    fetch: impl FnOnce(&Pup) -> Result<Vec<T>, PupError>,
) -> Loaded<T>
where
    T: Serialize + DeserializeOwned,
{
    let store = Store::new(&app.config.cache_dir);

    store.load(
        kind,
        &key(app, kind),
        app.now_ms,
        || fetch(&app.pup()?),
        || {
            let _ = app
                .runner
                .spawn_detached(&app.exe, &["refresh", kind.name()]);
        },
    )
}

pub fn load_dashboards(app: &App) -> Loaded<Board> {
    load(app, Kind::Dashboards, fetch_dashboards)
}

pub fn load_notebooks(app: &App) -> Loaded<Board> {
    load(app, Kind::Notebooks, fetch_notebooks)
}

/// Fetch `kind` now and replace its cache. `Ok(None)` when another refresh
/// is already running.
pub fn refresh(app: &App, kind: Kind) -> Result<Option<usize>, PupError> {
    let store = Store::new(&app.config.cache_dir);
    let key = key(app, kind);
    let pup = app.pup()?;

    match kind {
        Kind::Dashboards => store.refresh(kind, &key, app.now_ms, || fetch_dashboards(&pup)),
        Kind::Notebooks => store.refresh(kind, &key, app.now_ms, || fetch_notebooks(&pup)),
        Kind::Services | Kind::Monitors | Kind::Incidents => {
            Err(PupError::Other(format!("{kind} are not available yet")))
        }
    }
}

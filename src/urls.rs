//! Every Datadog web-app URL the workflow opens or copies. Pure.
//!
//! Base: `https://app.<DD_SITE>`. Times are epoch milliseconds: dashboards,
//! notebooks, logs and error tracking take `from_ts` / `to_ts` / `live`,
//! the Traces Explorer and the service page take `start` / `end` (and
//! `paused=false` to keep the range live).

use crate::percent::encode;

/// A time range ending at `to_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub from_ms: i64,
    pub to_ms: i64,
    /// The range ends now and keeps moving.
    pub live: bool,
}

impl Window {
    /// The last `length_ms`, ending at `now_ms`, live.
    pub fn last(length_ms: i64, now_ms: i64) -> Window {
        Window {
            from_ms: now_ms - length_ms,
            to_ms: now_ms,
            live: true,
        }
    }

    fn ts_params(&self) -> String {
        format!(
            "from_ts={}&to_ts={}&live={}",
            self.from_ms, self.to_ms, self.live
        )
    }

    fn start_end_params(&self) -> String {
        format!(
            "start={}&end={}&paused={}",
            self.from_ms, self.to_ms, !self.live
        )
    }
}

/// URL builder for one Datadog site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Urls {
    base: String,
}

impl Urls {
    pub fn new(site: &str) -> Urls {
        Urls {
            base: format!("https://app.{site}"),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// A dashboard, from the path the API returns
    /// (`/dashboard/<id>/<slug>`).
    pub fn dashboard(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// A dashboard on a time range.
    pub fn dashboard_at(&self, path: &str, window: Window) -> String {
        format!("{}{path}?{}", self.base, window.ts_params())
    }

    /// `path`, searched for `query` in its `q` parameter (none when blank).
    fn search(&self, path: &str, query: &str) -> String {
        let query = query.trim();

        if query.is_empty() {
            return format!("{}{path}", self.base);
        }

        format!("{}{path}?q={}", self.base, encode(query))
    }

    /// The dashboard list, searched for `query`.
    pub fn dashboard_search(&self, query: &str) -> String {
        self.search("/dashboard/lists", query)
    }

    pub fn notebook(&self, id: u64) -> String {
        format!("{}/notebook/{id}", self.base)
    }

    /// A notebook on a time range.
    pub fn notebook_at(&self, id: u64, window: Window) -> String {
        format!("{}/notebook/{id}?{}", self.base, window.ts_params())
    }

    /// The notebook list, searched for `query`.
    pub fn notebook_search(&self, query: &str) -> String {
        self.search("/notebook/list", query)
    }

    /// An APM service page in `env`.
    pub fn service(&self, name: &str, env: &str, window: Window) -> String {
        format!(
            "{}/apm/entity/service%3A{}?env={}&{}",
            self.base,
            encode(name),
            encode(env),
            window.start_end_params()
        )
    }

    /// The Log Explorer on `query`.
    pub fn logs(&self, query: &str, window: Window) -> String {
        format!(
            "{}/logs?query={}&{}",
            self.base,
            encode(query),
            window.ts_params()
        )
    }

    /// The Traces Explorer on `query`.
    pub fn traces(&self, query: &str, window: Window) -> String {
        format!(
            "{}/apm/traces?query={}&{}",
            self.base,
            encode(query),
            window.start_end_params()
        )
    }

    /// Error Tracking issues matching `query`.
    pub fn error_tracking(&self, query: &str, window: Window) -> String {
        format!(
            "{}/error-tracking?query={}&{}",
            self.base,
            encode(query),
            window.ts_params()
        )
    }

    pub fn monitor(&self, id: u64) -> String {
        format!("{}/monitors/{id}", self.base)
    }

    /// The monitor list, searched for `query`.
    pub fn monitor_search(&self, query: &str) -> String {
        self.search("/monitors/manage", query)
    }

    /// An incident, by its public id (the `#12` shown in the app).
    pub fn incident(&self, public_id: u64) -> String {
        format!("{}/incidents/{public_id}", self.base)
    }

    /// A trace, by its 128-bit hex or 64-bit decimal id.
    pub fn trace(&self, trace_id: &str) -> String {
        format!("{}/apm/trace/{}", self.base, encode(trace_id))
    }

    /// The same page on this site: `https://app.datadoghq.com/x` → `https://app.<site>/x`.
    /// `None` if `url` is not a Datadog app URL.
    pub fn on_this_site(&self, url: &str) -> Option<String> {
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))?;
        let (host, path) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));

        if !is_datadog_app_host(host) {
            return None;
        }

        Some(format!("{}{path}", self.base))
    }
}

/// `app.datadoghq.com`, `app.datadoghq.eu`, `us3.datadoghq.com`,
/// `app.ddog-gov.com`, `<subdomain>.datadoghq.com`…
pub fn is_datadog_app_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();

    ["datadoghq.com", "datadoghq.eu", "ddog-gov.com"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}

//! Read-only access to the Datadog API through the `pup` CLI.
//!
//! Every read is `pup --no-envelope api <endpoint>` (a raw GET returning the
//! API body as is). pup's dedicated subcommands (`dashboards list`,
//! `monitors search`, …) answer 403 with an OAuth token that `pup api`
//! accepts, so they are not used.
//!
//! pup refreshes an expired token on its own when it starts, but not a 401
//! returned mid-request: [`Pup::get`] then runs `pup auth refresh` once and
//! retries once. Other failures are classified into a [`PupError`] the script
//! filters turn into an item.

use std::fmt;
use std::path::Path;

use crate::runner::CommandRunner;

/// Where Homebrew installs pup (Apple Silicon first). Alfred does not load the
/// shell `PATH`, so pup is resolved by absolute path.
pub const CANDIDATES: [&str; 2] = ["/opt/homebrew/bin/pup", "/usr/local/bin/pup"];

/// Command that installs pup, shown when it is missing.
pub const INSTALL_COMMAND: &str = "brew install datadog-labs/pack/pup";

/// Command that logs pup in, shown when the session is gone.
pub const LOGIN_COMMAND: &str = "pup auth login";

/// Why a read failed, as far as the user can act on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PupError {
    /// pup is not installed (or could not be started).
    Missing,
    /// No session, or a session that cannot be refreshed: `pup auth login`.
    NotLoggedIn,
    /// The token lacks a permission for this endpoint.
    Forbidden,
    /// Datadog rate-limited the request.
    RateLimited,
    /// Anything else, with pup's first error line.
    Other(String),
}

impl fmt::Display for PupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PupError::Missing => write!(f, "pup is not installed ({INSTALL_COMMAND})"),
            PupError::NotLoggedIn => write!(f, "pup is not logged in (run `{LOGIN_COMMAND}`)"),
            PupError::Forbidden => write!(f, "Datadog refused the request (403)"),
            PupError::RateLimited => {
                write!(f, "Datadog rate limit reached, retry in a few seconds")
            }
            PupError::Other(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PupError {}

/// What a failed pup call means, before the refresh-and-retry decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The binary could not be spawned.
    Missing,
    /// No credentials at all.
    NotLoggedIn,
    /// 401 or an expired session: worth one `pup auth refresh`.
    Unauthorized,
    Forbidden,
    RateLimited,
    Other(String),
}

/// Classify the error message of a failed pup run. [`crate::runner`] puts the
/// exit status and pup's stderr in it.
pub fn classify(message: &str) -> Failure {
    let lower = message.to_lowercase();

    if lower.contains("failed to spawn") {
        return Failure::Missing;
    }

    if lower.contains("authentication required")
        || lower.contains("no tokens found")
        || lower.contains("no refresh token")
        || lower.contains("no client credentials")
    {
        return Failure::NotLoggedIn;
    }

    if lower.contains("authentication expired")
        || lower.contains("http 401")
        || lower.contains("status: 401")
    {
        return Failure::Unauthorized;
    }

    if lower.contains("http 403") || lower.contains("status: 403") {
        return Failure::Forbidden;
    }

    if lower.contains("http 429")
        || lower.contains("rate limited")
        || lower.contains("exit status: 173")
    {
        return Failure::RateLimited;
    }

    Failure::Other(first_error_line(message))
}

/// The most useful line of pup's output: the `Error: …` line if any, without
/// the response body, which can be long.
fn first_error_line(message: &str) -> String {
    let line = message
        .lines()
        .map(str::trim)
        .find(|line| line.contains("Error:"))
        .or_else(|| message.lines().map(str::trim).find(|line| !line.is_empty()))
        .unwrap_or("pup failed");
    let line = line.split_once("Error: ").map_or(line, |(_, rest)| rest);
    let line: String = line.chars().take(160).collect();

    line
}

impl From<Failure> for PupError {
    fn from(failure: Failure) -> PupError {
        match failure {
            Failure::Missing => PupError::Missing,
            Failure::NotLoggedIn | Failure::Unauthorized => PupError::NotLoggedIn,
            Failure::Forbidden => PupError::Forbidden,
            Failure::RateLimited => PupError::RateLimited,
            Failure::Other(message) => PupError::Other(message),
        }
    }
}

/// The installed pup, bound to the process boundary.
pub struct Pup<'a> {
    runner: &'a dyn CommandRunner,
    program: String,
}

impl<'a> Pup<'a> {
    /// pup at `program`.
    pub fn new(runner: &'a dyn CommandRunner, program: impl Into<String>) -> Pup<'a> {
        Pup {
            runner,
            program: program.into(),
        }
    }

    /// The first of [`CANDIDATES`] that exists, if any.
    pub fn locate(runner: &'a dyn CommandRunner) -> Option<Pup<'a>> {
        Pup::locate_in(runner, &CANDIDATES)
    }

    /// The first of `candidates` that is a file, if any.
    pub fn locate_in(runner: &'a dyn CommandRunner, candidates: &[&str]) -> Option<Pup<'a>> {
        candidates
            .iter()
            .find(|candidate| Path::new(candidate).is_file())
            .map(|program| Pup::new(runner, *program))
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    /// `GET /api/<endpoint>`: the raw response body.
    pub fn get(&self, endpoint: &str) -> Result<String, PupError> {
        match self.api(endpoint) {
            Ok(body) => Ok(body),
            Err(Failure::Unauthorized) => {
                if self.refresh_session().is_err() {
                    return Err(PupError::NotLoggedIn);
                }

                self.api(endpoint).map_err(PupError::from)
            }
            Err(failure) => Err(failure.into()),
        }
    }

    fn api(&self, endpoint: &str) -> Result<String, Failure> {
        self.runner
            .run(&self.program, &["--no-envelope", "api", endpoint])
            .map_err(|err| classify(&format!("{err:#}")))
    }

    fn refresh_session(&self) -> Result<(), Failure> {
        self.runner
            .run(&self.program, &["auth", "refresh"])
            .map(|_| ())
            .map_err(|err| classify(&format!("{err:#}")))
    }
}

//! What every subcommand needs: the process boundary, the configuration, the
//! programs it runs and the current time (injected so time-dependent logic
//! stays testable).

use crate::config::Config;
use crate::pup::{Pup, PupError};
use crate::runner::CommandRunner;
use crate::urls::Urls;

/// Shared context of one invocation of the binary.
pub struct App<'a> {
    pub runner: &'a dyn CommandRunner,
    pub config: Config,
    /// Absolute path of pup, if installed (see [`crate::pup::locate`]).
    pub pup_path: Option<String>,
    /// Path of the running `alfred-dd`, spawned for background refreshes.
    pub exe: String,
    /// Milliseconds since the Unix epoch.
    pub now_ms: i64,
}

impl<'a> App<'a> {
    pub fn new(
        runner: &'a dyn CommandRunner,
        config: Config,
        pup_path: Option<String>,
        exe: impl Into<String>,
        now_ms: i64,
    ) -> App<'a> {
        App {
            runner,
            config,
            pup_path,
            exe: exe.into(),
            now_ms,
        }
    }

    /// The installed pup, or [`PupError::Missing`].
    pub fn pup(&self) -> Result<Pup<'a>, PupError> {
        self.pup_path
            .as_deref()
            .map(|path| Pup::new(self.runner, path))
            .ok_or(PupError::Missing)
    }

    /// URL builder for the configured site.
    pub fn urls(&self) -> Urls {
        Urls::new(&self.config.site)
    }
}

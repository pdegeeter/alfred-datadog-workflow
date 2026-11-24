//! What every subcommand needs: the process boundary, the configuration and
//! the current time (injected so time-dependent logic stays testable).

use crate::config::Config;
use crate::runner::CommandRunner;

/// Shared context of one invocation of the binary.
pub struct App<'a> {
    pub runner: &'a dyn CommandRunner,
    pub config: Config,
    /// Milliseconds since the Unix epoch.
    pub now_ms: i64,
}

impl<'a> App<'a> {
    pub fn new(runner: &'a dyn CommandRunner, config: Config, now_ms: i64) -> App<'a> {
        App {
            runner,
            config,
            now_ms,
        }
    }
}

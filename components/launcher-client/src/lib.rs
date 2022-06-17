mod client;
pub mod error;

pub use habitat_launcher_protocol::{ERR_NO_RETRY_EXCODE,
                                    LAUNCHER_PID_ENV,
                                    OK_NO_RETRY_EXCODE};

pub use crate::{client::{LauncherCli,
                         LauncherStatus},
                error::*};

pub fn env_pipe() -> Option<String> {
    habitat_core::env::var(habitat_launcher_protocol::LAUNCHER_PIPE_ENV).ok()
}

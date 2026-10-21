//! What the command line reads about the machine it runs on.
//!
//! `language::environment` resolves the facts into the row install and `doctor` speak from, and
//! does no machine I/O itself; the reading happens here, at the command-line edge.

use std::env;
use std::fs;

use crate::language::environment::{self, Environment, EnvironmentFacts};

/// Where a Linux kernel reports its release string.
const KERNEL_RELEASE_PATH: &str = "/proc/sys/kernel/osrelease";

/// Read the environment facts of this process.
#[must_use]
pub fn observe() -> EnvironmentFacts {
    EnvironmentFacts {
        windows: cfg!(windows),
        wsl_distro: env::var("WSL_DISTRO_NAME").ok(),
        kernel_release: fs::read_to_string(KERNEL_RELEASE_PATH).ok(),
        current_desktop: env::var("XDG_CURRENT_DESKTOP").ok(),
    }
}

/// The environment row of the machine this process is running on.
#[must_use]
pub fn environment() -> Environment {
    environment::resolve(&observe())
}

//! A private Ghostlight home for tests that reach the service the way a peer does.
//!
//! A peer finds the service through `relay::home()`, which a test can only redirect with the
//! process-wide `GHOSTLIGHT_TEST_HOME`. Tests that rely on it hold one shared lock for their whole
//! run, so two of them never point the variable at different homes at once.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use ghostlight_relay::home::TEST_HOME_VARIABLE;

use crate::scratch::Scratch;
use crate::sync::lock;

/// Serializes every test that sets the home variable.
static HOME: Mutex<()> = Mutex::new(());

/// A temporary home the relay resolves to while this value lives.
pub(crate) struct TestHome {
    directory: Scratch,
    _exclusive: MutexGuard<'static, ()>,
}

impl TestHome {
    /// Create a fresh home named after `name` and point the relay at it.
    pub(crate) fn new(name: &str) -> Self {
        let exclusive = lock(&HOME);
        let directory = Scratch::named(&format!("home-{name}"));
        std::env::set_var(TEST_HOME_VARIABLE, directory.path());
        Self {
            directory,
            _exclusive: exclusive,
        }
    }

    /// The home directory.
    pub(crate) fn path(&self) -> &Path {
        self.directory.path()
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        std::env::remove_var(TEST_HOME_VARIABLE);
    }
}

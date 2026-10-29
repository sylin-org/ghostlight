//! What the update check says to the person, in the window's About page and on the command line
//! (ADR-0206). The check runs only when the person asks; these words never appear unprompted.

/// The About page's button and the CLI's command name the check by this.
pub const CHECK_LABEL: &str = "Check for updates";

/// Said while the check waits for GitHub.
pub const CHECKING: &str = "Asking GitHub for the latest release...";

/// Said when GitHub could not be reached; the person asks again when they want to.
pub const UNREACHABLE: &str = "Couldn't reach GitHub.";

/// Said when GitHub answered with something that is not a stable Ghostlight release.
pub const UNEXPECTED: &str = "GitHub did not answer with a Ghostlight release.";

/// Said when an administrator turned the check off with `updates.check.enabled`.
pub const TURNED_OFF: &str = "Your administrator turned off the update check.";

/// Said when the release has no images for this computer's kind.
pub const NO_IMAGES: &str = "The release has no Ghostlight images for this computer.";

/// Said when a download led to a host outside GitHub's release hosts.
pub const WRONG_HOST: &str =
    "A download led outside GitHub's release hosts, so nothing was installed.";

/// Said when a downloaded image did not match the release's checksum.
pub const CHECKSUM: &str =
    "A download did not match the release's checksums, so nothing was installed.";

/// Said when the staged images could not be written or the installer could not start.
pub const NOT_STAGED: &str =
    "The new version could not be prepared on this computer, so nothing was installed.";

/// The installed version is the latest, or newer than it.
#[must_use]
pub fn current(installed: &str) -> String {
    format!("Ghostlight {installed} is the latest.")
}

/// A newer release can be installed.
#[must_use]
pub fn newer(latest: &str) -> String {
    format!("Ghostlight {latest} is available.")
}

/// The button that installs a newer release.
#[must_use]
pub fn install_label(latest: &str) -> String {
    format!("Install {latest}")
}

/// What installing does to work in progress, said before the person installs.
#[must_use]
pub fn restart_warning(agents: usize) -> String {
    match agents {
        0 => "Installing restarts Ghostlight.".into(),
        1 => "Installing restarts Ghostlight. 1 agent is connected; it reconnects after.".into(),
        many => {
            format!("Installing restarts Ghostlight. {many} agents are connected; they reconnect after.")
        }
    }
}

/// A development build names the latest release and offers nothing.
#[must_use]
pub fn development(latest: &str) -> String {
    format!(
        "This is a development build. The latest release is {latest}; a development build updates through the dev loop."
    )
}

/// A newer release whose extension contract the connected browsers do not cover yet.
#[must_use]
pub fn needs_extension(latest: &str) -> String {
    format!(
        "Ghostlight {latest} needs a newer Ghostlight in Browser than your browser has. The Chrome Web Store installs it; check again once it has."
    )
}

/// What the command line says after naming a newer release, when it was not asked to install it.
pub const INSTALL_HINT: &str =
    "Run ghostlight update --install to install it. Installing restarts Ghostlight.";

/// What the command line says once the installer has started.
#[must_use]
pub fn installing_from_command_line(latest: &str) -> String {
    format!(
        "Installing Ghostlight {latest}. Ghostlight restarts; run ghostlight status to see it back."
    )
}

/// Said once the installer has started.
#[must_use]
pub fn installing(latest: &str) -> String {
    format!("Installing Ghostlight {latest}. Ghostlight restarts, and this window reconnects.")
}

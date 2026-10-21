//! What Ghostlight says about the machine it is running on.
//!
//! Install and `doctor` both have to tell a person where the product now lives and how it starts.
//! Those sentences differ by platform and by desktop shell: GNOME shows no tray, KDE does, Windows
//! has a notification area, and a harness running under WSL is looking at a browser that lives on
//! the other side of a boundary it cannot cross. One table composes them, so install and `doctor`
//! say the same thing.
//!
//! The vocabulary is one closed table with a row per environment, so adding a platform later is a
//! variant plus its phrases rather than a new branch at every call site. Each phrase is an
//! exhaustive match, so a new row does not compile until it has every phrase.

use std::path::Path;

/// The pinned marker that identifies a WSL kernel, matched case-insensitively.
const WSL_KERNEL_MARKER: &str = "microsoft";

/// What the service says when it is started with an elevated Windows token.
pub const ELEVATED_SERVICE: &str = "Ghostlight does not run with administrator rights. Start it from a regular terminal or from the Start menu.";

/// What an install says after it put the command on the Windows `Path`: a terminal that was open
/// before keeps the `Path` it started with.
pub const OPEN_NEW_TERMINAL: &str = "Open a new terminal to use ghostlight.";

/// What an install says when it put the `ghostlight` command at `command`, in a directory that is
/// not on the `PATH` it ran with: typing `ghostlight` there finds nothing, and the full path works.
#[must_use]
pub fn run_by_full_path(command: &Path) -> String {
    let directory = command.parent().unwrap_or(command);
    format!(
        "This terminal's PATH does not include {}, so run ghostlight by its full path: {}",
        directory.display(),
        command.display()
    )
}

/// What the service records when a browser or recovery start finds that the person quit
/// Ghostlight, and so does not bring it back.
pub const QUIT_HELD: &str = "Ghostlight was quit, so this start did not bring it back.";

/// What Ghostlight says when the machine will not let it listen for the end of the session. The
/// reason follows.
pub const SESSION_LISTENER_FAILED: &str = "Ghostlight could not listen for the end of the session:";

/// One row of the environment table.
///
/// Rows are distinguished only where Ghostlight has something different to say. A desktop shell
/// that shows a tray is not the same row as one that does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// Windows, which has a notification area and a Start menu.
    Windows,
    /// Linux running GNOME, which shows no status icons by default.
    LinuxGnome,
    /// Linux running KDE Plasma.
    LinuxKde,
    /// Linux running Xfce.
    LinuxXfce,
    /// Linux running Cinnamon.
    LinuxCinnamon,
    /// Linux running MATE.
    LinuxMate,
    /// Linux running a desktop Ghostlight does not recognize, or none.
    LinuxUnknown,
    /// A Linux environment inside WSL, where the browser lives on the Windows side.
    Wsl,
}

impl Environment {
    /// Where a person finds Ghostlight on this machine after installing it.
    ///
    /// Never promises a tray on a desktop that does not draw one. The Applications entry exists on
    /// every Linux install (ADR-0123), so it is always a truthful answer there.
    #[must_use]
    pub fn location(self) -> &'static str {
        match self {
            Environment::Windows => {
                "Ghostlight is in your notification area, and in the Start menu."
            }
            Environment::LinuxGnome => "Ghostlight is in your Applications menu.",
            Environment::LinuxKde
            | Environment::LinuxXfce
            | Environment::LinuxCinnamon
            | Environment::LinuxMate => {
                "Ghostlight is in your system tray and your Applications menu."
            }
            Environment::LinuxUnknown => {
                "Ghostlight is in your Applications menu, and in your system tray if your desktop draws one."
            }
            Environment::Wsl => {
                "Ghostlight is the `ghostlight` command in this WSL environment. It has no desktop entry here."
            }
        }
    }

    /// A caveat this environment needs, or `None` when it has nothing extra to say.
    ///
    /// WSL is the case that exists: the harness runs on the Linux side while the browser runs on
    /// Windows, and a browser cannot start a native messaging host across that boundary. Saying so
    /// is the whole remedy; Ghostlight does not attempt to bridge it.
    #[must_use]
    pub fn caveat(self) -> Option<&'static str> {
        match self {
            Environment::Wsl => Some(
                "You are running Ghostlight inside WSL. A browser on Windows cannot start a program inside WSL, so install Ghostlight on Windows to connect your browser there.",
            ),
            _ => None,
        }
    }

    /// The short name used in diagnostics.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Environment::Windows => "Windows",
            Environment::LinuxGnome => "Linux (GNOME)",
            Environment::LinuxKde => "Linux (KDE)",
            Environment::LinuxXfce => "Linux (Xfce)",
            Environment::LinuxCinnamon => "Linux (Cinnamon)",
            Environment::LinuxMate => "Linux (MATE)",
            Environment::LinuxUnknown => "Linux",
            Environment::Wsl => "Linux (WSL)",
        }
    }
}

/// The raw observations a row is resolved from.
///
/// Separated from resolution so the table can be tested without a real machine; the command line
/// reads them (`cli::machine`), and this module does no machine I/O.
#[derive(Debug, Default, Clone)]
pub struct EnvironmentFacts {
    /// Whether this build is running on Windows.
    pub windows: bool,
    /// The value of `WSL_DISTRO_NAME`, when set.
    pub wsl_distro: Option<String>,
    /// The kernel release string, when readable.
    pub kernel_release: Option<String>,
    /// The value of `XDG_CURRENT_DESKTOP`, when set.
    pub current_desktop: Option<String>,
}

impl EnvironmentFacts {
    fn is_wsl(&self) -> bool {
        if self
            .wsl_distro
            .as_deref()
            .is_some_and(|value| !value.is_empty())
        {
            return true;
        }
        self.kernel_release
            .as_deref()
            .is_some_and(|release| release.to_ascii_lowercase().contains(WSL_KERNEL_MARKER))
    }
}

/// Resolve one row from observed facts.
///
/// WSL wins over the desktop shell: a person there needs the boundary named before anything else.
#[must_use]
pub fn resolve(facts: &EnvironmentFacts) -> Environment {
    if facts.windows {
        return Environment::Windows;
    }
    if facts.is_wsl() {
        return Environment::Wsl;
    }
    let Some(desktop) = facts.current_desktop.as_deref() else {
        return Environment::LinuxUnknown;
    };
    desktop
        .split(':')
        .find_map(|entry| match entry.trim().to_ascii_lowercase().as_str() {
            "gnome" => Some(Environment::LinuxGnome),
            "kde" => Some(Environment::LinuxKde),
            "xfce" => Some(Environment::LinuxXfce),
            "cinnamon" => Some(Environment::LinuxCinnamon),
            "mate" => Some(Environment::LinuxMate),
            _ => None,
        })
        .unwrap_or(Environment::LinuxUnknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linux(desktop: Option<&str>) -> EnvironmentFacts {
        EnvironmentFacts {
            windows: false,
            wsl_distro: None,
            kernel_release: Some("6.9.3-arch1-1".to_owned()),
            current_desktop: desktop.map(str::to_owned),
        }
    }

    /// ADR-0126 Decision 3: Windows first, then WSL by `WSL_DISTRO_NAME` or a `microsoft`
    /// kernel marker, then the first recognized entry of `XDG_CURRENT_DESKTOP`, and an honest
    /// unknown row otherwise.
    #[test]
    fn the_row_follows_the_desktop_and_wsl_the_machine_shows() {
        let cases = [
            (
                EnvironmentFacts {
                    windows: true,
                    wsl_distro: Some("Ubuntu".to_owned()),
                    ..linux(Some("GNOME"))
                },
                Environment::Windows,
            ),
            (
                EnvironmentFacts {
                    wsl_distro: Some("Ubuntu-24.04".to_owned()),
                    ..linux(Some("GNOME"))
                },
                Environment::Wsl,
            ),
            (
                EnvironmentFacts {
                    kernel_release: Some("5.15.167.4-microsoft-standard-WSL2".to_owned()),
                    ..linux(Some("KDE"))
                },
                Environment::Wsl,
            ),
            (linux(Some("GNOME")), Environment::LinuxGnome),
            (linux(Some("ubuntu:GNOME")), Environment::LinuxGnome),
            (linux(Some("KDE")), Environment::LinuxKde),
            (linux(Some("XFCE")), Environment::LinuxXfce),
            (
                linux(Some("X-Cinnamon:Cinnamon")),
                Environment::LinuxCinnamon,
            ),
            (linux(Some("MATE")), Environment::LinuxMate),
            (linux(None), Environment::LinuxUnknown),
            (linux(Some("")), Environment::LinuxUnknown),
            (linux(Some("sway")), Environment::LinuxUnknown),
            (linux(Some("Hyprland:wlroots")), Environment::LinuxUnknown),
        ];
        for (facts, expected) in cases {
            assert_eq!(resolve(&facts), expected, "for {facts:?}");
        }
    }
}

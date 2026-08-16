//! What Ghostlight says about the machine it is running on.
//!
//! Install and `doctor` both have to tell a person where the product now lives and how it starts.
//! Those sentences differ by platform and by desktop shell: GNOME shows no tray, KDE does, Windows
//! has a notification area, and a harness running under WSL is looking at a browser that lives on
//! the other side of a boundary it cannot cross. Before this module each call site would have had
//! to compose that itself, and the two would have drifted.
//!
//! The vocabulary is one closed table with a row per environment, so adding a platform later is a
//! variant plus its phrases rather than a new branch at every call site. A guard test proves every
//! row has every phrase.

use std::env;
use std::fs;

/// The pinned marker that identifies a WSL kernel, matched case-insensitively.
const WSL_KERNEL_MARKER: &str = "microsoft";
/// Where a Linux kernel reports its release string.
const KERNEL_RELEASE_PATH: &str = "/proc/sys/kernel/osrelease";

/// What Ghostlight says once, at the end of a successful install, about running in the background.
///
/// A person arriving from a platform where this class of tool starts at login will otherwise read
/// the absence of a resident process as a defect.
pub const BACKGROUND_POSTURE: &str = "Ghostlight starts when your agent or your browser asks for it. Nothing runs in the background until then.";

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
    /// Every row, in table order. Exhaustiveness guards iterate this.
    pub const ALL: &'static [Environment] = &[
        Environment::Windows,
        Environment::LinuxGnome,
        Environment::LinuxKde,
        Environment::LinuxXfce,
        Environment::LinuxCinnamon,
        Environment::LinuxMate,
        Environment::LinuxUnknown,
        Environment::Wsl,
    ];

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
/// Separated from resolution so the table can be tested without a real machine.
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
    /// Read the facts from this process's environment.
    #[must_use]
    pub fn observe() -> Self {
        Self {
            windows: cfg!(windows),
            wsl_distro: env::var("WSL_DISTRO_NAME").ok(),
            kernel_release: fs::read_to_string(KERNEL_RELEASE_PATH).ok(),
            current_desktop: env::var("XDG_CURRENT_DESKTOP").ok(),
        }
    }

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

/// Resolve the row for the machine this process is running on.
#[must_use]
pub fn current() -> Environment {
    resolve(&EnvironmentFacts::observe())
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

    #[test]
    fn resolves_wsl_from_distro_environment() {
        let facts = EnvironmentFacts {
            wsl_distro: Some("Ubuntu-24.04".to_owned()),
            ..linux(Some("GNOME"))
        };
        assert_eq!(resolve(&facts), Environment::Wsl);
    }

    #[test]
    fn resolves_wsl_from_kernel_release_marker() {
        let facts = EnvironmentFacts {
            kernel_release: Some("5.15.167.4-microsoft-standard-WSL2".to_owned()),
            ..linux(Some("KDE"))
        };
        assert_eq!(resolve(&facts), Environment::Wsl);
    }

    #[test]
    fn resolves_each_recognized_linux_shell() {
        for (value, expected) in [
            ("GNOME", Environment::LinuxGnome),
            ("ubuntu:GNOME", Environment::LinuxGnome),
            ("KDE", Environment::LinuxKde),
            ("XFCE", Environment::LinuxXfce),
            ("X-Cinnamon:Cinnamon", Environment::LinuxCinnamon),
            ("MATE", Environment::LinuxMate),
        ] {
            assert_eq!(resolve(&linux(Some(value))), expected, "for {value}");
        }
    }

    #[test]
    fn unrecognized_desktop_falls_back_to_the_unknown_row() {
        for value in [None, Some(""), Some("sway"), Some("Hyprland:wlroots")] {
            assert_eq!(
                resolve(&linux(value)),
                Environment::LinuxUnknown,
                "for {value:?}"
            );
        }
    }

    #[test]
    fn every_environment_row_has_a_location_phrase() {
        for row in Environment::ALL {
            assert!(!row.location().is_empty(), "{row:?} has no location phrase");
            assert!(!row.label().is_empty(), "{row:?} has no label");
        }
        // A row that claims a tray must not be one of the shells that does not draw one.
        assert!(!Environment::LinuxGnome.location().contains("tray"));
        assert!(Environment::LinuxKde.location().contains("tray"));
    }

    #[test]
    fn background_posture_sentence_matches_the_pinned_value() {
        assert_eq!(
            BACKGROUND_POSTURE,
            "Ghostlight starts when your agent or your browser asks for it. Nothing runs in the background until then."
        );
    }

    #[test]
    fn only_wsl_carries_a_caveat() {
        for row in Environment::ALL {
            let caveat = row.caveat();
            if *row == Environment::Wsl {
                assert!(caveat.is_some_and(|text| text.contains("WSL")));
            } else {
                assert!(caveat.is_none(), "{row:?} should have no caveat");
            }
        }
    }

    #[test]
    fn windows_wins_before_any_linux_observation() {
        let facts = EnvironmentFacts {
            windows: true,
            wsl_distro: Some("Ubuntu".to_owned()),
            ..linux(Some("GNOME"))
        };
        assert_eq!(resolve(&facts), Environment::Windows);
    }
}

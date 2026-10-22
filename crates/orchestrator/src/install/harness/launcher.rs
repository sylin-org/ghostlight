//! Ghostlight's own npm launcher as an entry may start it: never current, and only Fix
//! replaces it (ADR-0203 Decision 10.3).

use super::*;

/// The program that runs an npm package.
pub(super) const NPX_PROGRAM: &str = "npx";

/// The Windows command interpreter some clients start `npx` through, and its switch that runs the
/// command after it.
pub(super) const WINDOWS_SHELL: (&str, &str) = ("cmd", "/c");

/// The `npx` options a launcher entry may carry before the package. They answer npm's own
/// prompts and choose nothing.
pub(super) const NPX_QUIET_OPTIONS: [&str; 4] = ["-y", "--yes", "-q", "--quiet"];

/// The npm package Ghostlight's launcher is published as.
pub(super) const NPM_PACKAGE: &str = "ghostlight";

/// The npm tag for whichever version npm serves as the newest.
pub(super) const NPM_LATEST_TAG: &str = "latest";

/// The characters besides letters and digits a version, range or tag after `ghostlight@` is
/// written with. A spec with any other character, such as a URL or a path, is not the launcher.
pub(super) const NPM_SPEC_MARKS: &str = ".-+^~=*<>";

/// The first major release whose npm launcher starts a Ghostlight that works with this one: 2.0
/// changed the relay and the extension's protocol.
pub const FIRST_LAUNCHER_MAJOR: u64 = 2;

/// Which version Ghostlight's npm launcher entry asks npm for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LauncherPin {
    /// No version, the `latest` tag or another name npm resolves: npm chooses the version at each
    /// launch.
    Unpinned,
    /// A release before 2.0, such as 1.x, whose launcher starts a Ghostlight that cannot work
    /// with this one.
    Earlier,
    /// A 2.0 or later version or range.
    Pinned,
}

impl LauncherPin {
    /// The pin the package `spec` after `ghostlight@` names, or none.
    pub(super) fn of(spec: Option<&str>) -> Self {
        let Some(spec) = spec.filter(|spec| !spec.is_empty() && *spec != NPM_LATEST_TAG) else {
            return Self::Unpinned;
        };
        let version = spec.trim_start_matches(['^', '~', '=', 'v']);
        let major: String = version.chars().take_while(char::is_ascii_digit).collect();
        match major.parse::<u64>() {
            Ok(major) if major < FIRST_LAUNCHER_MAJOR => Self::Earlier,
            Ok(_) => Self::Pinned,
            Err(_) => Self::Unpinned,
        }
    }
}

/// Ghostlight's own npm launcher as an entry starts it: `npx` with only quiet options, the
/// `ghostlight` package, and no command or `mcp`, through `cmd /c` or not.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NpmLauncher {
    /// Which version the entry asks npm for.
    pub pin: LauncherPin,
    /// The package as the entry names it, `ghostlight` or `ghostlight@<version>`, bounded for
    /// disclosure.
    pub package: String,
}

impl NpmLauncher {
    /// The launcher an entry starting `command` with `args` names, or none when it starts anything
    /// else. A command that names an existing file is that program whole; any other command is
    /// read as a whole command line.
    pub(super) fn from_entry(command: &str, args: Option<&[&str]>) -> Option<Self> {
        let mut words: Vec<&str> = if Path::new(command).is_file() {
            vec![command]
        } else {
            command.split_whitespace().collect()
        };
        words.extend(args.unwrap_or_default().iter().copied());
        let mut words = words.into_iter().peekable();
        if words
            .peek()
            .is_some_and(|word| command_name(word, WINDOWS_SHELL.0))
        {
            words.next();
            if !words
                .next()
                .is_some_and(|word| word.eq_ignore_ascii_case(WINDOWS_SHELL.1))
            {
                return None;
            }
        }
        if !words
            .next()
            .is_some_and(|word| command_name(word, NPX_PROGRAM))
        {
            return None;
        }
        let package = words.find(|word| !NPX_QUIET_OPTIONS.contains(word))?;
        let spec = match package.strip_prefix(NPM_PACKAGE)? {
            "" => None,
            rest => Some(rest.strip_prefix('@')?),
        };
        if spec.is_some_and(|spec| {
            !spec
                .chars()
                .all(|mark| mark.is_ascii_alphanumeric() || NPM_SPEC_MARKS.contains(mark))
        }) {
            return None;
        }
        let rest: Vec<&str> = words.collect();
        if !(rest.is_empty() || rest == [MCP_ARGUMENT]) {
            return None;
        }
        Some(Self {
            pin: LauncherPin::of(spec),
            package: bounded_disclosure(package)?,
        })
    }
}

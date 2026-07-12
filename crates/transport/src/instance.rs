// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The named-instance identity derivation (ADR-0044): one instance name is the single source of
//! truth for every stack-identity string, so a machine can run more than one isolated Ghostlight
//! stack (a `dev` alongside the default deploy) without collision.
//!
//! ## The one rule that guards everything
//! The DEFAULT instance (an unnamed [`Instance::default`]) MUST reproduce every current identifier
//! BYTE-FOR-BYTE, or the shipped product, existing installs, and the install/fidelity tests break
//! (ADR-0044 Decision 2). The pin test in this module ([`tests::default_instance_is_byte_identical`])
//! is the guard that exists before any former-constant call site moves onto this derivation.
//!
//! ## How it is resolved (ADR-0044 Decision 1)
//! Identity is resolved from a single named parameter, by this precedence:
//! 1. the `--instance <name>` global CLI flag (the human running `install`/`doctor`/etc.),
//! 2. the `GHOSTLIGHT_INSTANCE` env var (tests, the e2e harness, and the value `main` folds the
//!    winner into so every point-of-use derivation agrees),
//! 3. the `argv[0]` basename ([`Instance::from_exe_stem`]) -- a binary named `ghostlight-<name>` is
//!    instance `<name>`; this is the ONLY signal Chrome's arg-free native-host launch can carry
//!    (ADR-0044 Decision 4, the multi-call binary),
//! 4. the canonical default.
//!
//! `main` reconciles that precedence once at startup and writes the winner back into
//! `GHOSTLIGHT_INSTANCE`, so every derivation site can simply call [`Instance::resolve`] -- the same
//! "resolve from the environment at the point of use" convention the tree already uses for
//! `GHOSTLIGHT_ENDPOINT`, `GHOSTLIGHT_LOG_DIR`, and `GHOSTLIGHT_USER_CONFIG_DIR`.
//!
//! ## Isolation, not profiles (ADR-0044 Decision 3 + Risks)
//! An instance is ONLY an identity plus isolated directories. It is NOT a place to hang behavioral
//! config (that is the layered config's job). Every derived string is a pure function of the name.

/// The reverse-DNS base shared by the native-host name, the IPC endpoint, and the macOS supervisor
/// label. The default instance yields exactly this; a named instance appends a `.<name>` segment.
const REVERSE_DNS_BASE: &str = "org.sylin.ghostlight";

/// The short leaf base shared by the MCP server name, the config/policy/log directory leaf, and the
/// Linux supervisor unit. The default instance yields exactly this; a named instance appends a
/// `-<name>` suffix.
const LEAF_BASE: &str = "ghostlight";

/// The IPC endpoint's trailing version segment (`org.sylin.ghostlight[.<name>].v1`).
const ENDPOINT_VERSION: &str = "v1";

/// The Windows Task Scheduler display base (`Ghostlight Service[ (<name>)]`).
const SERVICE_DISPLAY_BASE: &str = "Ghostlight Service";

/// Maximum instance-name length. Kept short so `org.sylin.ghostlight.<name>.v1` stays comfortably
/// within OS socket-path limits (the Unix socket layer additionally hashes any overflow).
pub const MAX_INSTANCE_NAME_LEN: usize = 32;

/// A resolved Ghostlight stack identity. `name == None` is the canonical DEFAULT instance, whose
/// derivations are byte-identical to the single-instance identifiers the product shipped with; a
/// `Some(name)` is an isolated non-default instance.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Instance {
    /// `None` is the canonical default instance; `Some(name)` is an isolated named instance. Always
    /// a validated name (see [`Instance::from_name`]) when present.
    name: Option<String>,
}

impl Instance {
    /// The environment seam that carries the resolved instance to every point-of-use derivation
    /// and to Chrome-launched native-host processes (ADR-0044 Decision 1, Decision 4).
    pub const ENV_VAR: &str = "GHOSTLIGHT_INSTANCE";

    /// Validate a candidate instance name (ADR-0044 Decision 3 security posture): the name flows
    /// into filesystem paths, socket/pipe names, Windows registry keys, and OS supervisor unit
    /// names, so it is a system boundary that must be validated. Accepts lowercase ASCII letters,
    /// digits, and hyphens; must start with a letter and not end with a hyphen; length
    /// `1..=`[`MAX_INSTANCE_NAME_LEN`]. Rejects the reserved word `default` (omit `--instance` for
    /// the default instance). This rules out path separators, `..`, dots, whitespace, and uppercase,
    /// so no derived path can traverse or collide by case-folding.
    pub fn validate(name: &str) -> std::result::Result<(), String> {
        if name.eq_ignore_ascii_case("default") {
            return Err(
                "the instance name 'default' is reserved; omit --instance to use the default \
                 instance"
                    .to_string(),
            );
        }
        let len = name.len();
        if len == 0 || len > MAX_INSTANCE_NAME_LEN {
            return Err(format!(
                "an instance name must be 1..={MAX_INSTANCE_NAME_LEN} characters (got {len})"
            ));
        }
        let bytes = name.as_bytes();
        let well_formed = bytes[0].is_ascii_lowercase()
            && bytes[len - 1] != b'-'
            && bytes
                .iter()
                .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !well_formed {
            return Err(format!(
                "invalid instance name '{name}': use lowercase letters, digits, and hyphens; start \
                 with a letter; do not end with a hyphen (e.g. 'dev', 'qa-staging')"
            ));
        }
        Ok(())
    }

    /// Build a named instance from a validated name, or return the validation error verbatim.
    pub fn from_name(name: &str) -> std::result::Result<Self, String> {
        Self::validate(name)?;
        Ok(Self {
            name: Some(name.to_string()),
        })
    }

    /// Resolve the active instance from [`ENV_VAR`] at the point of use (the same convention as
    /// `native::ipc::default_endpoint`). An unset or empty value is the default instance. An
    /// invalid value falls back to the default with a warning rather than poisoning every path:
    /// `main` validates strictly up front ([`Instance::validate_env`]), so a real process never
    /// reaches here with an invalid value -- this leniency is only a library/test safety net.
    pub fn resolve() -> Self {
        match std::env::var(Self::ENV_VAR) {
            Ok(raw) if !raw.trim().is_empty() => {
                let name = raw.trim();
                Self::from_name(name).unwrap_or_else(|_| {
                    tracing::warn!(
                        value = %name,
                        "ignoring an invalid GHOSTLIGHT_INSTANCE; using the default instance"
                    );
                    Self::default()
                })
            }
            _ => Self::default(),
        }
    }

    /// Resolve the instance from the running executable's file name (ADR-0044 Decision 4, the
    /// multi-call signal): a binary named `ghostlight-<name>` is instance `<name>`; the bare
    /// `ghostlight` is the default. Returns `None` when the name carries NO instance signal -- an
    /// unrelated basename (a renamed binary), or a `ghostlight-<x>` whose `<x>` fails validation --
    /// so the caller can fall through to another source. `file_stem` drops the trailing `.exe` on
    /// Windows and leaves a bare Unix name intact. This is the ONLY instance signal Chrome's
    /// arg-free native-host launch can carry.
    pub fn from_exe_stem(exe: &std::path::Path) -> Option<Self> {
        Self::from_exe_stem_with_base(exe, LEAF_BASE)
    }

    /// [`from_exe_stem`] generalized over the executable's base name (ADR-0046: a role executable
    /// resolves argv[0] against ITS OWN base, so `ghostlight-relay` is that bin's DEFAULT instance,
    /// never a bogus instance named "relay"; the browser role uses this on the ADR-0051 Phase 3
    /// per-instance `ghostlight-relay-<n>` copy Chrome launches by name).
    pub fn from_exe_stem_with_base(exe: &std::path::Path, base: &str) -> Option<Self> {
        let stem = exe.file_stem()?.to_str()?;
        if stem == base {
            return Some(Self::default());
        }
        let name = stem.strip_prefix(base)?.strip_prefix('-')?;
        Self::from_name(name).ok()
    }

    /// Strictly validate whatever [`ENV_VAR`] currently holds (if anything). Called once at
    /// `main` startup so a malformed instance -- notably from a broken native-host wrapper -- fails
    /// fast with a clear message instead of silently degrading to the default. An unset/empty var
    /// is fine (the default instance).
    pub fn validate_env() -> std::result::Result<(), String> {
        match std::env::var(Self::ENV_VAR) {
            Ok(raw) if !raw.trim().is_empty() => Self::validate(raw.trim()),
            _ => Ok(()),
        }
    }

    /// The instance name, or `None` for the default instance.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// True for the canonical default instance (the byte-identical, unsuffixed identity).
    pub fn is_default(&self) -> bool {
        self.name.is_none()
    }

    /// A human label for reports (`doctor`): the instance name, or `default`.
    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or("default")
    }

    /// The reverse-DNS shape: `org.sylin.ghostlight` (default) or `org.sylin.ghostlight.<name>`.
    fn reverse_dns(&self) -> String {
        match &self.name {
            None => REVERSE_DNS_BASE.to_string(),
            Some(n) => format!("{REVERSE_DNS_BASE}.{n}"),
        }
    }

    /// The short-leaf shape: `ghostlight` (default) or `ghostlight-<name>`.
    fn leaf(&self) -> String {
        match &self.name {
            None => LEAF_BASE.to_string(),
            Some(n) => format!("{LEAF_BASE}-{n}"),
        }
    }

    /// The IPC endpoint base name: `org.sylin.ghostlight.v1` (default) or
    /// `org.sylin.ghostlight.<name>.v1` (`native::ipc`).
    pub fn endpoint(&self) -> String {
        format!("{}.{ENDPOINT_VERSION}", self.reverse_dns())
    }

    /// The Chrome native-messaging host name: `org.sylin.ghostlight` (default) or
    /// `org.sylin.ghostlight.<name>` (`transport::native::host`, `install::native_host`).
    pub fn host_name(&self) -> String {
        self.reverse_dns()
    }

    /// The MCP server name advertised to clients and used as the client-config entry key:
    /// `ghostlight` (default) or `ghostlight-<name>` (`install::clients`, `transport::mcp::server`).
    pub fn mcp_server_name(&self) -> String {
        self.leaf()
    }

    /// The Windows Task Scheduler task name: `Ghostlight Service` (default) or
    /// `Ghostlight Service (<name>)` (`hub::supervisor`, `install::supervisor`).
    pub fn supervisor_task_name(&self) -> String {
        match &self.name {
            None => SERVICE_DISPLAY_BASE.to_string(),
            Some(n) => format!("{SERVICE_DISPLAY_BASE} ({n})"),
        }
    }

    /// The macOS launchd label: `org.sylin.ghostlight.service` (default) or
    /// `org.sylin.ghostlight.<name>.service` (`hub::supervisor`, `install::supervisor`).
    pub fn supervisor_label(&self) -> String {
        format!("{}.service", self.reverse_dns())
    }

    /// The Linux systemd --user unit: `ghostlight.service` (default) or `ghostlight-<name>.service`
    /// (`hub::supervisor`, `install::supervisor`).
    pub fn supervisor_unit(&self) -> String {
        format!("{}.service", self.leaf())
    }

    /// The config / policy / log directory leaf: `ghostlight` (default) or `ghostlight-<name>`
    /// (`governance::config::load`, `observability`, `install::native_host`). A non-default
    /// instance's user config, org policy, and observability files never touch the default's.
    pub fn dir_leaf(&self) -> String {
        self.leaf()
    }
}

/// Classify one raw instance source (a `--instance` value or the env var's content) into an
/// [`Instance`] (ADR-0064: every client pins exactly one instance; there is no "resolve at connect
/// time, prefer dev" state). Pure (no environment access), so it is unit-testable without racing
/// parallel tests over process-global env state: `None`/blank is the DEFAULT instance; the reserved
/// word `default` (any case) is the default; a valid name is that named instance; anything else
/// returns the validation error verbatim.
pub fn classify(source: Option<&str>) -> std::result::Result<Instance, String> {
    match source.map(str::trim) {
        None | Some("") => Ok(Instance::default()),
        Some(s) if s.eq_ignore_ascii_case("default") => Ok(Instance::default()),
        Some(s) => Instance::from_name(s),
    }
}

/// Resolve a client's instance from an optional `--instance` flag value, falling back to
/// [`Instance::ENV_VAR`] (a blank flag value is treated as absent), and NORMALIZE the environment so
/// every downstream point-of-use [`Instance::resolve`] agrees: a NAMED instance writes its name
/// back; the default REMOVES the variable. Replaces the ADR-0048 `Selection`/`Unpinned` shadow
/// (ADR-0064): absent means the default instance, connected to explicitly -- not "prefer a live dev".
pub fn resolve_from(flag: Option<&str>) -> std::result::Result<Instance, String> {
    let env = std::env::var(Instance::ENV_VAR).ok();
    let source = match flag.map(str::trim) {
        Some(f) if !f.is_empty() => Some(f.to_string()),
        _ => env,
    };
    let instance = classify(source.as_deref())?;
    match instance.name() {
        Some(name) => std::env::set_var(Instance::ENV_VAR, name),
        None => std::env::remove_var(Instance::ENV_VAR),
    }
    Ok(instance)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-0044 Decision 2, the guard the whole change rests on: the default instance reproduces
    /// every shipped identifier BYTE-FOR-BYTE. If any of these ever changes, the published product,
    /// existing installs, and the install/fidelity tests break.
    #[test]
    fn default_instance_is_byte_identical() {
        let d = Instance::default();
        assert!(d.is_default());
        assert_eq!(d.name(), None);
        assert_eq!(d.label(), "default");
        assert_eq!(d.endpoint(), "org.sylin.ghostlight.v1");
        assert_eq!(d.host_name(), "org.sylin.ghostlight");
        assert_eq!(d.mcp_server_name(), "ghostlight");
        assert_eq!(d.supervisor_task_name(), "Ghostlight Service");
        assert_eq!(d.supervisor_label(), "org.sylin.ghostlight.service");
        assert_eq!(d.supervisor_unit(), "ghostlight.service");
        assert_eq!(d.dir_leaf(), "ghostlight");
    }

    /// A named instance suffixes every identifier per the ADR-0044 derivation table, so a `dev`
    /// stack is isolated from the default at every identity point at once.
    #[test]
    fn a_named_instance_suffixes_every_identifier() {
        let dev = Instance::from_name("dev").unwrap();
        assert!(!dev.is_default());
        assert_eq!(dev.name(), Some("dev"));
        assert_eq!(dev.label(), "dev");
        assert_eq!(dev.endpoint(), "org.sylin.ghostlight.dev.v1");
        assert_eq!(dev.host_name(), "org.sylin.ghostlight.dev");
        assert_eq!(dev.mcp_server_name(), "ghostlight-dev");
        assert_eq!(dev.supervisor_task_name(), "Ghostlight Service (dev)");
        assert_eq!(dev.supervisor_label(), "org.sylin.ghostlight.dev.service");
        assert_eq!(dev.supervisor_unit(), "ghostlight-dev.service");
        assert_eq!(dev.dir_leaf(), "ghostlight-dev");
    }

    /// A hyphenated name flows through unambiguously: derivations always compose against the KNOWN
    /// base, never re-parse, so an internal hyphen (`qa-staging`) is fine.
    #[test]
    fn a_hyphenated_name_composes_against_the_known_base() {
        let qa = Instance::from_name("qa-staging").unwrap();
        assert_eq!(qa.endpoint(), "org.sylin.ghostlight.qa-staging.v1");
        assert_eq!(qa.mcp_server_name(), "ghostlight-qa-staging");
        assert_eq!(qa.dir_leaf(), "ghostlight-qa-staging");
        assert_eq!(qa.supervisor_unit(), "ghostlight-qa-staging.service");
    }

    #[test]
    fn validation_accepts_reasonable_names() {
        for good in ["dev", "qa", "a", "x1", "qa-staging", "release-candidate-2"] {
            assert!(Instance::from_name(good).is_ok(), "should accept {good:?}");
        }
    }

    #[test]
    fn validation_rejects_dangerous_or_malformed_names() {
        for bad in [
            "",        // empty
            "Dev",     // uppercase
            "DEV",     // uppercase
            "1dev",    // leading digit
            "-dev",    // leading hyphen
            "dev-",    // trailing hyphen
            "de v",    // whitespace
            "a.b",     // dot would look like a nested reverse-dns segment
            "a/b",     // path separator
            "a\\b",    // path separator
            "../evil", // path traversal
            "default", // reserved
            "DEFAULT", // reserved (case-insensitive)
        ] {
            assert!(Instance::from_name(bad).is_err(), "should reject {bad:?}");
        }
        // Over the length cap.
        assert!(Instance::from_name(&"x".repeat(MAX_INSTANCE_NAME_LEN + 1)).is_err());
        // Exactly the cap is allowed.
        assert!(Instance::from_name(&"x".repeat(MAX_INSTANCE_NAME_LEN)).is_ok());
    }

    #[test]
    fn the_reserved_default_error_points_at_omitting_the_flag() {
        let err = Instance::from_name("default").unwrap_err();
        assert!(err.contains("reserved"));
        assert!(err.contains("omit --instance"));
    }

    #[test]
    fn from_exe_stem_reads_the_multi_call_name() {
        use std::path::Path;
        // Forward-slash paths are separator-valid on every platform (a backslash is NOT a
        // separator on Unix, so Windows-style literals here would break the Linux/macOS CI).
        assert!(Instance::from_exe_stem(Path::new("/usr/bin/ghostlight"))
            .unwrap()
            .is_default());
        assert!(Instance::from_exe_stem(Path::new("/x/ghostlight.exe"))
            .unwrap()
            .is_default());
        // A ghostlight-<n> copy resolves to <n>.
        assert_eq!(
            Instance::from_exe_stem(Path::new("/x/ghostlight-dev.exe"))
                .unwrap()
                .name(),
            Some("dev")
        );
        assert_eq!(
            Instance::from_exe_stem(Path::new("/opt/ghostlight-qa-staging"))
                .unwrap()
                .name(),
            Some("qa-staging")
        );
        // Windows-style separators, on Windows only.
        #[cfg(windows)]
        {
            assert!(Instance::from_exe_stem(Path::new(r"C:\x\ghostlight.exe"))
                .unwrap()
                .is_default());
            assert_eq!(
                Instance::from_exe_stem(Path::new(r"C:\x\ghostlight-dev.exe"))
                    .unwrap()
                    .name(),
                Some("dev")
            );
        }
        // No instance signal: an unrelated basename, or an invalid <n> (dot, leading digit).
        assert!(Instance::from_exe_stem(Path::new("/usr/bin/some-other-tool")).is_none());
        assert!(Instance::from_exe_stem(Path::new("/x/ghostlight-1.2.3.exe")).is_none());
        assert!(Instance::from_exe_stem(Path::new("/x/ghostlight-.exe")).is_none());
    }

    #[test]
    fn from_exe_stem_with_base_resolves_the_relay_family() {
        use std::path::Path;
        // Forward-slash paths only: a backslash is NOT a separator on Unix, so a Windows-style
        // literal here would break the Linux/macOS CI (this exact mistake reddened CI once already).
        let base = "ghostlight-relay";
        assert!(
            Instance::from_exe_stem_with_base(Path::new("/x/ghostlight-relay"), base)
                .unwrap()
                .is_default()
        );
        assert_eq!(
            Instance::from_exe_stem_with_base(Path::new("/x/ghostlight-relay-dev.exe"), base)
                .unwrap()
                .name(),
            Some("dev")
        );
        assert_eq!(
            Instance::from_exe_stem_with_base(Path::new("/x/ghostlight-relay-qa-staging"), base)
                .unwrap()
                .name(),
            Some("qa-staging")
        );
        // The bare `ghostlight` binary is NOT in this family.
        assert!(Instance::from_exe_stem_with_base(Path::new("/x/ghostlight"), base).is_none());
    }

    /// ADR-0064: classify maps one raw source to exactly one instance -- absent/blank/`default` is
    /// the default; a valid name is that instance; garbage errors. There is no "unpinned/prefer-dev"
    /// state anymore.
    #[test]
    fn classify_maps_a_source_to_exactly_one_instance() {
        assert!(classify(None).unwrap().is_default());
        assert!(classify(Some("")).unwrap().is_default());
        assert!(classify(Some("  ")).unwrap().is_default());
        assert!(classify(Some("default")).unwrap().is_default());
        assert!(classify(Some("DEFAULT")).unwrap().is_default());
        assert_eq!(classify(Some("dev")).unwrap().name(), Some("dev"));
        assert_eq!(
            classify(Some("qa-staging")).unwrap().name(),
            Some("qa-staging")
        );
        assert!(classify(Some("Not Valid")).is_err());
    }
}

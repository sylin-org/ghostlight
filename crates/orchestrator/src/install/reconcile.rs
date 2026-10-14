//! Registrations the installer reconciles after every install.
//!
//! Every registration names an executable in `home/bin` (ADR-0197 Decisions 13 and 14): the
//! browsers' native-host manifests name `bin/ghostlight-browser-connector`, owned MCP client
//! entries name `bin/ghostlight-mcp-connector`, and the platform entries (the Linux desktop entry,
//! command link and manual pages; the Windows Start-menu shortcut and Uninstall key) name
//! `bin/ghostlight`. Reconciling is idempotent: what is current is left alone. A native-host
//! manifest that names another path is reclaimed; a client entry Ghostlight does not own, such
//! as an `npx` launcher, is reported and left as it is.
//!
//! Inside a Windows app container the registry and `AppData` writes reach only that app, so the
//! reconcile writes the manifest (below the home, which is shared) and leaves every registration
//! that would need a redirected write, naming each one in [`ReconcileReport::kept_private`].

use std::env;

use ghostlight_relay::home::TEST_HOME_VARIABLE;
use serde::Serialize;

use super::command_path::CommandPath;
use super::container::{AppContainer, Residence};
use super::desktop_entry::DesktopIntegration;
use super::layout::Layout;
use super::migration::RegistrationKind;
use super::native_host::{NativeHostRegistry, NativeHostState, NATIVE_HOST_DIR_VARIABLE};
use super::user_assets::UserAssets;
#[cfg(windows)]
use super::windows_entry::{WindowsEntryChange, INSTALLED_APPS_ENTRY, START_MENU_SHORTCUT};
use super::{HarnessAction, HarnessRegistry, HarnessState};

/// A registration the next reconcile will write.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PendingRegistration {
    /// What kind of registration this is.
    pub kind: RegistrationKind,
    /// The browser or client identifier.
    pub id: String,
    /// The registration's state now, in plain words.
    pub state: String,
}

/// A client entry under Ghostlight's name that the reconcile did not change.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UnchangedEntry {
    /// The client's name.
    pub client: String,
    /// The entry found, bounded, when there is one to show.
    pub found: Option<String>,
}

/// What one reconcile changed and what it left for a person.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ReconcileReport {
    /// Browsers whose native-host registration was written.
    pub browsers: Vec<String>,
    /// Clients whose Ghostlight entry was rewritten to the installed connector.
    pub clients_rewritten: Vec<String>,
    /// Client entries under Ghostlight's name that Ghostlight does not own.
    pub clients_unchanged: Vec<UnchangedEntry>,
    /// Platform entries that were written, in plain words.
    pub platform: Vec<String>,
    /// Registrations that could not be reconciled, each with its reason.
    pub failures: Vec<String>,
    /// The app container the reconcile ran in, when it had to leave registrations unwritten.
    pub container: Option<AppContainer>,
    /// Registrations left unwritten because the container keeps their registry or `AppData`
    /// location private; a regular terminal finishes them.
    pub kept_private: Vec<String>,
}

/// The registrations of this user that name the installation, and which of them the installer
/// may change.
pub struct Registrations {
    native_hosts: Option<NativeHostRegistry>,
    clients: Option<HarnessRegistry>,
    platform_entries: bool,
    residence: Residence,
}

impl Registrations {
    /// This user's registrations.
    ///
    /// Under a test home (`GHOSTLIGHT_TEST_HOME`) only native-host registrations rooted by
    /// `GHOSTLIGHT_NATIVE_HOST_DIR` are reconciled, so a journey never rewrites the registrations
    /// of the person running it.
    #[must_use]
    pub fn discover() -> Self {
        let test_home = env::var_os(TEST_HOME_VARIABLE).is_some();
        Self {
            native_hosts: native_hosts_allowed().then(NativeHostRegistry::discover),
            clients: (!test_home).then(HarnessRegistry::discover),
            platform_entries: !test_home,
            residence: Residence::detect(),
        }
    }

    /// Registrations over the given native hosts and clients, without platform entries, for
    /// tests that must not touch the machine.
    #[cfg(test)]
    pub(crate) fn isolated(native_hosts: NativeHostRegistry, clients: HarnessRegistry) -> Self {
        Self {
            native_hosts: Some(native_hosts),
            clients: Some(clients),
            platform_entries: false,
            residence: Residence::Regular,
        }
    }

    /// The same registrations, run as if from inside `container`.
    #[cfg(test)]
    pub(crate) fn inside(mut self, container: AppContainer) -> Self {
        self.residence = Residence::AppContainer(container);
        self
    }

    /// The registrations a reconcile would write now. Nothing is changed.
    #[must_use]
    pub fn pending(&self) -> Vec<PendingRegistration> {
        let mut pending = Vec::new();
        if let Some(report) = self
            .native_hosts
            .as_ref()
            .and_then(|hosts| hosts.check().ok())
        {
            pending.extend(
                report
                    .browsers
                    .into_iter()
                    .filter(|browser| {
                        matches!(
                            browser.state,
                            NativeHostState::Missing
                                | NativeHostState::Updatable
                                | NativeHostState::OwnedElsewhere
                        )
                    })
                    .map(|browser| PendingRegistration {
                        kind: RegistrationKind::NativeHost,
                        id: browser.id,
                        state: browser.state.label().to_owned(),
                    }),
            );
        }
        if let Some(clients) = &self.clients {
            pending.extend(
                clients
                    .summaries()
                    .into_iter()
                    .filter(|summary| summary.state == HarnessState::Updatable)
                    .map(|summary| PendingRegistration {
                        kind: RegistrationKind::McpClient,
                        id: summary.id,
                        state: summary.state.label().to_owned(),
                    }),
            );
        }
        pending
    }

    /// Point every registration at the installation in `layout`, whose service is `version`.
    ///
    /// A registration that cannot be written is reported in the result and does not stop the
    /// others.
    pub fn reconcile(&self, layout: &Layout, version: &str) -> ReconcileReport {
        let mut report = ReconcileReport::default();
        if let Some(hosts) = &self.native_hosts {
            reconcile_native_hosts(hosts, &mut report);
        }
        if let Some(clients) = &self.clients {
            reconcile_clients(clients, &mut report);
        }
        if self.platform_entries {
            reconcile_platform(layout, version, &self.residence, &mut report);
        }
        if !report.kept_private.is_empty() {
            report.container = self.residence.container().cloned();
        }
        report
    }
}

/// Whether this process may change native-host registrations: always for a real home, and under
/// a test home only when `GHOSTLIGHT_NATIVE_HOST_DIR` roots them somewhere else.
#[must_use]
pub fn native_hosts_allowed() -> bool {
    env::var_os(TEST_HOME_VARIABLE).is_none() || env::var_os(NATIVE_HOST_DIR_VARIABLE).is_some()
}

/// Write every supported browser's manifest, reclaiming one that names another connector.
/// Manifests Ghostlight does not own are left for a person.
fn reconcile_native_hosts(hosts: &NativeHostRegistry, report: &mut ReconcileReport) {
    let before = match hosts.check() {
        Ok(before) => before,
        Err(error) => {
            report.failures.push(format!("Browser connection: {error}"));
            return;
        }
    };
    let changing: Vec<String> = before
        .browsers
        .iter()
        .filter(|browser| {
            matches!(
                browser.state,
                NativeHostState::Missing
                    | NativeHostState::Updatable
                    | NativeHostState::OwnedElsewhere
            )
        })
        .map(|browser| browser.name.clone())
        .collect();
    if changing.is_empty() {
        return;
    }
    match hosts.install() {
        Ok(result) => {
            report.browsers = changing
                .into_iter()
                .filter(|name| !result.private.contains(name))
                .collect();
            report.kept_private.extend(
                result
                    .private
                    .iter()
                    .map(|name| format!("{name} browser connection")),
            );
        }
        Err(error) => report.failures.push(format!("Browser connection: {error}")),
    }
}

/// Rewrite the owned client entries that name another connector, and report the entries under
/// Ghostlight's name that Ghostlight does not own.
fn reconcile_clients(clients: &HarnessRegistry, report: &mut ReconcileReport) {
    let summaries = match clients.refresh() {
        Ok(summaries) => summaries,
        Err(error) => {
            report.failures.push(format!("MCP clients: {error}"));
            return;
        }
    };
    for summary in summaries {
        match summary.state {
            HarnessState::Updatable if summary.private_view => report
                .kept_private
                .push(format!("{} MCP client", summary.label)),
            HarnessState::Updatable => match clients.apply(&summary.id, HarnessAction::Install) {
                Ok(_) => report.clients_rewritten.push(summary.label),
                Err(error) => report.failures.push(format!("{}: {error}", summary.label)),
            },
            HarnessState::NeedsAttention => report.clients_unchanged.push(UnchangedEntry {
                client: summary.label,
                found: summary.evidence,
            }),
            HarnessState::NotDetected | HarnessState::Available | HarnessState::Installed => {}
        }
    }
}

/// Write the platform's own entries for the installed service.
fn reconcile_platform(
    layout: &Layout,
    version: &str,
    residence: &Residence,
    report: &mut ReconcileReport,
) {
    match CommandPath::discover().install() {
        Ok(result) if result.changed => report
            .platform
            .push(format!("command link: {}", result.report.state.label())),
        Ok(_) => {}
        Err(error) => report.failures.push(format!("Command link: {error}")),
    }
    match UserAssets::discover().install() {
        Ok(result) if result.changed => report
            .platform
            .push(format!("manual pages: {}", result.report.state.label())),
        Ok(_) => {}
        Err(error) => report.failures.push(format!("Manual pages: {error}")),
    }
    match DesktopIntegration::discover().install() {
        Ok(result) if result.changed => report.platform.push(format!(
            "applications entry: {}",
            result.report.state.label()
        )),
        Ok(_) => {}
        Err(error) => report.failures.push(format!("Applications entry: {error}")),
    }
    reconcile_windows(layout, version, residence, report);
}

/// Write the Start-menu shortcut and the Uninstall key.
///
/// Inside an app container both would land in that app's private view, so neither is written;
/// each one that is out of date is named in [`ReconcileReport::kept_private`], and the next
/// install from a regular terminal writes it.
#[cfg(windows)]
fn reconcile_windows(
    layout: &Layout,
    version: &str,
    residence: &Residence,
    report: &mut ReconcileReport,
) {
    reconcile_entries(
        residence,
        report,
        || super::windows_entry::plan(layout, version),
        || super::windows_entry::reconcile(layout, version),
    );
}

/// Apply `apply` to the Windows entries, or, inside an app container, only name what `plan` says
/// is out of date.
#[cfg(windows)]
fn reconcile_entries(
    residence: &Residence,
    report: &mut ReconcileReport,
    plan: impl FnOnce() -> std::io::Result<WindowsEntryChange>,
    apply: impl FnOnce() -> std::io::Result<WindowsEntryChange>,
) {
    if residence.container().is_some() {
        match plan() {
            Ok(change) => {
                if change.shortcut {
                    report.kept_private.push(START_MENU_SHORTCUT.to_owned());
                }
                if change.uninstall_key {
                    report.kept_private.push(INSTALLED_APPS_ENTRY.to_owned());
                }
            }
            Err(error) => report.failures.push(format!("Start menu: {error}")),
        }
        return;
    }
    match apply() {
        Ok(change) => {
            if change.shortcut {
                report.platform.push(START_MENU_SHORTCUT.to_owned());
            }
            if change.nsis_removed {
                report
                    .platform
                    .push("installed-apps entry of an earlier installer removed".to_owned());
            }
            if change.uninstall_key {
                report.platform.push(INSTALLED_APPS_ENTRY.to_owned());
            }
        }
        Err(error) => report.failures.push(format!("Start menu: {error}")),
    }
}

/// Other platforms have no Start menu or Uninstall key.
#[cfg(not(windows))]
fn reconcile_windows(
    _layout: &Layout,
    _version: &str,
    _residence: &Residence,
    _report: &mut ReconcileReport,
) {
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::{json, Value};

    use super::super::layout::{BROWSER_CONNECTOR_EXECUTABLE_NAME, MCP_CONNECTOR_EXECUTABLE_NAME};
    use super::super::native_host::HOST_NAME;
    use super::super::scratch::Scratch;
    use super::*;

    /// Browser whose manifest the tests plant.
    const BROWSER: &str = "chrome";

    struct Fixture {
        _scratch: Scratch,
        layout: Layout,
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let scratch = Scratch::new();
            let root = fs::canonicalize(scratch.path()).unwrap();
            let layout = Layout::new(root.join("home"));
            fs::create_dir_all(layout.bin()).unwrap();
            fs::write(layout.browser_connector(), b"browser").unwrap();
            fs::write(layout.mcp_connector(), b"mcp").unwrap();
            Self {
                _scratch: scratch,
                layout,
                root,
            }
        }

        fn private_registrations(&self, app: &str) -> Registrations {
            Registrations::isolated(
                NativeHostRegistry::isolated(
                    &self.root.join("hosts"),
                    &self.layout.browser_connector(),
                ),
                HarnessRegistry::isolated_inside(
                    &self.root.join("clients"),
                    &self.layout.mcp_connector(),
                    &self.layout.state(),
                    AppContainer { app: app.into() },
                ),
            )
            .inside(AppContainer { app: app.into() })
        }

        fn registrations(&self) -> Registrations {
            Registrations::isolated(
                NativeHostRegistry::isolated(
                    &self.root.join("hosts"),
                    &self.layout.browser_connector(),
                ),
                HarnessRegistry::isolated(
                    &self.root.join("clients"),
                    &self.layout.mcp_connector(),
                    &self.layout.state(),
                ),
            )
        }

        fn manifest(&self) -> PathBuf {
            NativeHostRegistry::isolated(&self.root.join("hosts"), &self.layout.browser_connector())
                .isolated_manifest(BROWSER)
        }

        fn client_config(&self, relative: &str) -> PathBuf {
            relative
                .split('/')
                .fold(self.root.join("clients"), |path, part| path.join(part))
        }

        fn write_json(path: &Path, value: &Value) {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
        }
    }

    #[test]
    fn a_manifest_naming_another_connector_is_reclaimed() {
        let fixture = Fixture::new();
        let old = fixture
            .root
            .join("old")
            .join(BROWSER_CONNECTOR_EXECUTABLE_NAME);
        Fixture::write_json(
            &fixture.manifest(),
            &json!({
                "name": HOST_NAME,
                "description": "Ghostlight browser connector",
                "path": old,
                "type": "stdio",
                "allowed_origins": [],
            }),
        );
        let registrations = fixture.registrations();
        assert!(registrations
            .pending()
            .iter()
            .any(|pending| pending.kind == RegistrationKind::NativeHost && pending.id == BROWSER));
        let report = registrations.reconcile(&fixture.layout, "2.0.0");
        assert!(report.failures.is_empty(), "{report:?}");
        assert!(!report.browsers.is_empty());
        let manifest: Value =
            serde_json::from_str(&fs::read_to_string(fixture.manifest()).unwrap()).unwrap();
        assert_eq!(
            fs::canonicalize(manifest["path"].as_str().unwrap()).unwrap(),
            fs::canonicalize(fixture.layout.browser_connector()).unwrap()
        );
        let again = fixture.registrations().reconcile(&fixture.layout, "2.0.0");
        assert!(again.browsers.is_empty(), "{again:?}");
    }

    #[test]
    fn an_owned_client_entry_is_rewritten_and_an_npx_entry_is_left_alone() {
        let fixture = Fixture::new();
        let owned = fixture.client_config("config/Claude/claude_desktop_config.json");
        let old_connector = fixture.root.join("old").join(MCP_CONNECTOR_EXECUTABLE_NAME);
        Fixture::write_json(
            &owned,
            &json!({"mcpServers": {"ghostlight": {"command": old_connector}}}),
        );
        let npx = fixture.client_config("home/.cline/data/settings/cline_mcp_settings.json");
        let npx_entry =
            json!({"mcpServers": {"ghostlight": {"command": "npx", "args": ["-y", "ghostlight"]}}});
        Fixture::write_json(&npx, &npx_entry);
        let npx_before = fs::read(&npx).unwrap();

        let report = fixture.registrations().reconcile(&fixture.layout, "2.0.0");
        assert!(report.failures.is_empty(), "{report:?}");
        assert_eq!(report.clients_rewritten, ["Claude Desktop"]);
        assert_eq!(report.clients_unchanged.len(), 1, "{report:?}");
        assert!(report.clients_unchanged[0]
            .found
            .as_deref()
            .is_some_and(|found| found.contains("npx")));
        assert_eq!(fs::read(&npx).unwrap(), npx_before);
        let rewritten: Value = serde_json::from_str(&fs::read_to_string(&owned).unwrap()).unwrap();
        assert_eq!(
            Path::new(
                rewritten["mcpServers"]["ghostlight"]["command"]
                    .as_str()
                    .unwrap()
            ),
            fixture.layout.mcp_connector()
        );
    }

    #[test]
    fn inside_another_apps_container_a_client_under_appdata_is_reported_private_and_left_alone() {
        let fixture = Fixture::new();
        let owned = fixture.client_config("config/Claude/claude_desktop_config.json");
        let old_connector = fixture.root.join("old").join(MCP_CONNECTOR_EXECUTABLE_NAME);
        Fixture::write_json(
            &owned,
            &json!({"mcpServers": {"ghostlight": {"command": old_connector}}}),
        );
        let before = fs::read(&owned).unwrap();

        let report = fixture
            .private_registrations("OpenAI.Codex")
            .reconcile(&fixture.layout, "2.0.0");

        assert!(report.failures.is_empty(), "{report:?}");
        assert!(report.clients_rewritten.is_empty(), "{report:?}");
        assert_eq!(report.kept_private, ["Claude Desktop MCP client"]);
        assert_eq!(
            report.container,
            Some(AppContainer {
                app: "OpenAI.Codex".into()
            })
        );
        assert_eq!(fs::read(&owned).unwrap(), before);
    }

    #[test]
    fn inside_its_own_container_a_packaged_client_is_rewritten() {
        let fixture = Fixture::new();
        let owned = fixture.client_config("config/Claude/claude_desktop_config.json");
        let old_connector = fixture.root.join("old").join(MCP_CONNECTOR_EXECUTABLE_NAME);
        Fixture::write_json(
            &owned,
            &json!({"mcpServers": {"ghostlight": {"command": old_connector}}}),
        );

        let report = fixture
            .private_registrations("Claude")
            .reconcile(&fixture.layout, "2.0.0");

        assert!(report.failures.is_empty(), "{report:?}");
        assert_eq!(report.clients_rewritten, ["Claude Desktop"]);
        assert!(report.kept_private.is_empty(), "{report:?}");
        assert_eq!(report.container, None);
    }

    #[cfg(windows)]
    mod windows_entries {
        use std::io;

        use super::super::super::windows_entry::WindowsEntryChange;
        use super::super::*;

        const STALE: WindowsEntryChange = WindowsEntryChange {
            shortcut: false,
            uninstall_key: true,
            nsis_removed: false,
        };

        fn container() -> Residence {
            Residence::AppContainer(AppContainer {
                app: "Claude".into(),
            })
        }

        #[test]
        fn inside_a_container_a_stale_entry_is_named_and_never_written() {
            let mut report = ReconcileReport::default();
            reconcile_entries(
                &container(),
                &mut report,
                || {
                    Ok(WindowsEntryChange {
                        shortcut: true,
                        ..STALE
                    })
                },
                || panic!("a container must not write its private view"),
            );
            assert_eq!(
                report.kept_private,
                ["Start-menu shortcut", "installed-apps entry"]
            );
            assert!(report.platform.is_empty(), "{report:?}");
        }

        #[test]
        fn inside_a_container_current_entries_name_nothing() {
            let mut report = ReconcileReport::default();
            reconcile_entries(
                &container(),
                &mut report,
                || Ok(WindowsEntryChange::default()),
                || panic!("a container must not write its private view"),
            );
            assert!(report.kept_private.is_empty(), "{report:?}");
        }

        #[test]
        fn inside_a_container_an_unreadable_entry_is_a_failure() {
            let mut report = ReconcileReport::default();
            reconcile_entries(
                &container(),
                &mut report,
                || Err(io::Error::other("no registry")),
                || panic!("a container must not write its private view"),
            );
            assert_eq!(report.failures, ["Start menu: no registry"]);
        }

        #[test]
        fn outside_a_container_the_entries_are_written_and_reported() {
            let mut report = ReconcileReport::default();
            reconcile_entries(
                &Residence::Regular,
                &mut report,
                || panic!("a regular install writes without planning"),
                || Ok(STALE),
            );
            assert_eq!(report.platform, ["installed-apps entry"]);
            assert!(report.kept_private.is_empty(), "{report:?}");
        }
    }

    #[test]
    fn a_regular_reconcile_reports_nothing_private() {
        let fixture = Fixture::new();
        let report = fixture.registrations().reconcile(&fixture.layout, "2.0.0");
        assert!(report.kept_private.is_empty(), "{report:?}");
        assert_eq!(report.container, None);
    }
}

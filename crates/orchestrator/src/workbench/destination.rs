//! The closed vocabulary of places outside Ghostlight the workbench is willing to open, and the
//! address of each.

use serde::Deserialize;

/// Every place the workbench is willing to send someone.
///
/// The surface names a destination; the orchestrator owns the address. A closed vocabulary is the
/// difference between "the workbench can open a link" and "the workbench can open any URL": there
/// is no free string to smuggle a destination through, so a page that is not on this list cannot
/// be reached from here however the surface is coaxed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchDestination {
    /// The Ghostlight project page.
    Home,
    /// The live demo stage.
    Demo,
    /// The guided "which mode fits me" aid.
    DecisionAid,
    /// The source repository.
    Source,
    /// Installing Ghostlight and connecting a client.
    InstallGuide,
    /// Writing and applying a governance manifest.
    GovernanceGuide,
    /// Driving Ghostlight from a script or shell.
    ScriptingGuide,
    /// Shipping the audit trail into a SIEM.
    AuditGuide,
    /// What the product promises, and how each claim is checked.
    TrustCenter,
    /// Every architectural decision, and why it was taken.
    DecisionRecords,
    /// The licensing split in plain language.
    LicensingGuide,
    /// The rest of the Sylin toolkit.
    SylinTools,
    /// Google's download page for Chrome, which the setup view offers a person with no browser
    /// Ghostlight supports.
    GetChrome,
}

/// The branch of the source repository that holds the documentation of the published product.
/// A person running a release reads the documents of the product they run, and `main` is that
/// branch from the cutover on.
macro_rules! documentation_branch {
    () => {
        "main"
    };
}

/// One document of the source repository, on the documentation branch.
macro_rules! repository_document {
    ($path:literal) => {
        concat!(
            "https://github.com/sylin-org/ghostlight/blob/",
            documentation_branch!(),
            "/",
            $path
        )
    };
}

impl WorkbenchDestination {
    /// Resolve the exact address. Every one of these was checked to resolve when it was added.
    ///
    /// Documentation links name the branch of `documentation_branch!`, so a release shows the
    /// documents of the product it is.
    #[must_use]
    pub const fn url(self) -> &'static str {
        match self {
            Self::Home => "https://sylin.org/ghostlight/",
            Self::Demo => "https://sylin.org/ghostlight/demo/",
            Self::DecisionAid => "https://sylin.org/ghostlight/decision-aid/",
            Self::Source => "https://github.com/sylin-org/ghostlight",
            Self::InstallGuide => {
                repository_document!("docs/guides/installation.md")
            }
            Self::GovernanceGuide => {
                repository_document!("docs/guides/governance-configuration.md")
            }
            Self::ScriptingGuide => {
                repository_document!("docs/guides/scripting.md")
            }
            Self::AuditGuide => {
                repository_document!("docs/guides/siem-integration.md")
            }
            Self::TrustCenter => {
                repository_document!("docs/trust/README.md")
            }
            Self::DecisionRecords => {
                repository_document!("docs/adr/README.md")
            }
            Self::LicensingGuide => {
                repository_document!("docs/guides/licensing.md")
            }
            Self::SylinTools => "https://sylin.org/tools/",
            Self::GetChrome => "https://www.google.com/chrome/",
        }
    }

    /// The full vocabulary, so a new destination cannot be added without the guards seeing it.
    #[must_use]
    pub const fn all() -> [Self; 13] {
        [
            Self::Home,
            Self::Demo,
            Self::DecisionAid,
            Self::Source,
            Self::InstallGuide,
            Self::GovernanceGuide,
            Self::ScriptingGuide,
            Self::AuditGuide,
            Self::TrustCenter,
            Self::DecisionRecords,
            Self::LicensingGuide,
            Self::SylinTools,
            Self::GetChrome,
        ]
    }

    /// The token the surface uses to name this destination.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Demo => "demo",
            Self::DecisionAid => "decision_aid",
            Self::Source => "source",
            Self::InstallGuide => "install_guide",
            Self::GovernanceGuide => "governance_guide",
            Self::ScriptingGuide => "scripting_guide",
            Self::AuditGuide => "audit_guide",
            Self::TrustCenter => "trust_center",
            Self::DecisionRecords => "decision_records",
            Self::LicensingGuide => "licensing_guide",
            Self::SylinTools => "sylin_tools",
            Self::GetChrome => "get_chrome",
        }
    }
}

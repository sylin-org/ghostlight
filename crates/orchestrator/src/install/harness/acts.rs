//! The acts a person or the reconcile asks for: install, fix and uninstall, each through the
//! one edit of the client's resolved format.

use super::*;

pub(super) fn apply_install(
    context: &HarnessContext,
    definition: &ResolvedHarness,
) -> Result<HarnessActionResult, HarnessError> {
    refuse_private(context, definition)?;
    if !context.edge.is_file() {
        return Err(HarnessError::EdgeMissing);
    }
    let changed = edit_at(
        context,
        definition,
        &definition.path,
        RegistrationEdit::Install,
    )?;
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        message: if changed {
            words::client_registered(&summary.label)
        } else {
            words::client_already_current(&summary.label)
        },
        summary,
    })
}

/// Remove Ghostlight's entry from its configuration and also from every other copy of the
/// agent's settings that holds an entry Ghostlight owns, since an earlier install may have written
/// either. A copy that holds no owned entry, or cannot be read, is left as it is. When only other
/// copies hold one, the configuration itself is left as it is too, so a foreign entry or an
/// unreadable text there does not stop their cleanup.
pub(super) fn apply_uninstall(
    context: &HarnessContext,
    definition: &ResolvedHarness,
) -> Result<HarnessActionResult, HarnessError> {
    refuse_private(context, definition)?;
    let others = owned_other_copies(context, definition);
    let mut changed =
        if others.is_empty() || holds_owned_entry(context, definition, &definition.path) {
            uninstall_at(context, definition, &definition.path)?
        } else {
            false
        };
    for other in others {
        changed |= uninstall_at(context, definition, other)?;
    }
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        message: if changed {
            words::client_removed(&summary.label)
        } else {
            words::client_had_no_entry(&summary.label)
        },
        summary,
    })
}

/// The other copies of the agent's settings, besides the configuration itself, that hold an entry
/// Ghostlight owns. Uninstall removes Ghostlight's entry from each, and inspection names them, so
/// what a dry run says and what Uninstall does come from this one reading.
pub(super) fn owned_other_copies<'a>(
    context: &HarnessContext,
    definition: &'a ResolvedHarness,
) -> Vec<&'a Path> {
    definition
        .other_copies(context.windows)
        .filter(|path| holds_owned_entry(context, definition, path))
        .collect()
}

/// Whether the file at `path`, read as the configuration of `definition`, holds an entry
/// Ghostlight owns, current or not.
pub(super) fn holds_owned_entry(
    context: &HarnessContext,
    definition: &ResolvedHarness,
    path: &Path,
) -> bool {
    registration_state_at(context, definition, path).is_ok_and(|state| state.is_owned())
}

/// Remove Ghostlight's own entry from the file at `path`, read as the configuration of
/// `definition`. Whether the file changed.
pub(super) fn uninstall_at(
    context: &HarnessContext,
    definition: &ResolvedHarness,
    path: &Path,
) -> Result<bool, HarnessError> {
    edit_at(context, definition, path, RegistrationEdit::Uninstall)
}

/// Make the edit `intent` to Ghostlight's entry in the file at `path`, read as the configuration
/// of `definition`. Whether the file changed. A client whose file Ghostlight never writes is
/// refused with its manual setup.
pub(super) fn edit_at(
    context: &HarnessContext,
    definition: &ResolvedHarness,
    path: &Path,
    intent: RegistrationEdit,
) -> Result<bool, HarnessError> {
    match definition.dialect.format(context, || read_or_empty(path))? {
        Ok(format) => format.edit(context, path, intent),
        Err(_) => Err(manual_refusal(definition)),
    }
}

pub(super) fn apply_fix(
    context: &HarnessContext,
    definition: &ResolvedHarness,
) -> Result<HarnessActionResult, HarnessError> {
    refuse_private(context, definition)?;
    if !context.edge.is_file() {
        return Err(HarnessError::EdgeMissing);
    }
    let launcher = matches!(
        registration_state(context, definition),
        Ok(RegistrationState::Launcher(_))
    );
    let changed = edit_at(context, definition, &definition.path, RegistrationEdit::Fix)?;
    let summary = inspect(context, definition);
    Ok(HarnessActionResult {
        changed,
        message: if launcher {
            words::launcher_fixed(&summary.label)
        } else {
            words::client_fixed(&summary.label)
        },
        summary,
    })
}

/// Refuse a change that would reach only the private view of the app container this process runs
/// in.
pub(super) fn refuse_private(
    context: &HarnessContext,
    definition: &ResolvedHarness,
) -> Result<(), HarnessError> {
    match context.private_to(definition) {
        Some(container) => Err(HarnessError::PrivateView(container.app.clone())),
        None => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RegistrationEdit {
    Install,
    Uninstall,
    Fix,
}

impl RegistrationEdit {
    pub(super) const fn installs(self) -> bool {
        !matches!(self, Self::Uninstall)
    }

    pub(super) const fn replaces_foreign(self) -> bool {
        matches!(self, Self::Fix)
    }
}

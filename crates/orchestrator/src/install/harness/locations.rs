//! The paths a person located for a harness: kept owner-private in the home and checked
//! before they are used.

use super::*;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct LocationOverride {
    pub(super) executable: Option<PathBuf>,
    pub(super) config: Option<PathBuf>,
}

pub(super) type LocationOverrides = BTreeMap<String, LocationOverride>;

pub(super) fn read_location_overrides(
    context: &HarnessContext,
) -> Result<LocationOverrides, HarnessError> {
    match fs::read_to_string(&context.locations) {
        Ok(source) => serde_json::from_str(&source)
            .map_err(|error| HarnessError::MalformedLocationState(error.to_string())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(HarnessError::Read(error)),
    }
}

/// Write the located paths owner-only: they are a record about the person, not the client's file,
/// so they take Ghostlight's own private replace and no backup copy beside them.
pub(super) fn write_location_overrides(
    context: &HarnessContext,
    overrides: &LocationOverrides,
) -> Result<(), HarnessError> {
    let bytes = serde_json::to_vec_pretty(overrides)
        .map_err(|error| HarnessError::MalformedLocationState(error.to_string()))?;
    files::atomic_replace(&context.locations, &bytes).map_err(HarnessError::Write)
}

pub(super) fn validate_located_config(
    path: &Path,
    dialect: ConfigDialect,
) -> Result<(), HarnessError> {
    let toml = match dialect {
        ConfigDialect::Manual(_) => return Ok(()),
        ConfigDialect::CodexToml => true,
        ConfigDialect::Json(_) | ConfigDialect::OpenCode => false,
    };
    let source = fs::read_to_string(path).map_err(HarnessError::Read)?;
    if toml {
        parse_toml(&source).map(|_| ())
    } else {
        parse_jsonc(&source).map(|_| ())
    }
}

pub(super) fn is_executable_file(path: &Path, windows: bool) -> bool {
    if !path.is_file() {
        return false;
    }
    if windows {
        return path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|extension| {
                WINDOWS_PROGRAM_SUFFIX
                    .strip_prefix('.')
                    .is_some_and(|suffix| extension.eq_ignore_ascii_case(suffix))
            });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

//! Where a Microsoft Store app keeps its own copy of an agent's settings, found on every look.
//!
//! Windows runs a Store (MSIX) app in a container that sends its writes below `%APPDATA%` and
//! `%LOCALAPPDATA%` to a private folder of its package, `%LOCALAPPDATA%\Packages\<family>\
//! LocalCache\Roaming` and `...\LocalCache\Local`, and the app reads that copy first. Ghostlight
//! runs outside any container, so it looks there itself instead of trusting a table of package
//! names: for a configuration below either root, the candidates are the plain path and the same
//! relative path below each installed package's private copy of that root. The evidence is the
//! client's own settings folder, not only its file, so a fresh Store install that has not written
//! the file yet is still found.
//!
//! A look lists `Packages` once, keeps only the packages that have a private copy of a root, and
//! then tests the one settings folder each client names in each of them. Nothing is searched
//! recursively and the registry is not read. The order of the copies is the order the agent reads
//! them in ([`Packages::copies`]): a copy in a package of the agent's own Store app, named as the
//! catalog and the Store lookup of [`super::icons`] name it, beats the plain path. Another
//! package's copy never does, because only a process inside that other app's container reads it:
//! it is listed after the plain path, to be named and cleaned up, and never chosen (ADR-0197
//! amendments "Packaged clients seen from outside", 2026-10-17, and "Another app's container is
//! not the agent's", 2026-10-20).

use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::container::PACKAGE_FIELD_SEPARATOR;

/// Where the packages installed for the person keep their data, below `%LOCALAPPDATA%`: one folder
/// per package family, named by the family name.
pub(super) const PACKAGES_DIRECTORY: &str = "Packages";

/// The folder of a package's data that holds its private copies of the data roots.
const LOCAL_CACHE: &str = "LocalCache";

/// How many entries of [`PACKAGES_DIRECTORY`] a look reads at most.
const MAX_PACKAGES: usize = 4096;

/// One of the two folders a Store app's container redirects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DataRoot {
    /// `%APPDATA%`, the roaming application data.
    Roaming,
    /// `%LOCALAPPDATA%`, the local application data.
    Local,
}

/// Every data root, in the order a configuration path is matched against them.
const DATA_ROOTS: [DataRoot; 2] = [DataRoot::Roaming, DataRoot::Local];

impl DataRoot {
    /// The folder below a package's [`LOCAL_CACHE`] that holds its private copy of this root.
    const fn folder(self) -> &'static str {
        match self {
            Self::Roaming => "Roaming",
            Self::Local => "Local",
        }
    }
}

/// The folder where the package family `family` keeps its private copy of `root`, below the
/// person's `%LOCALAPPDATA%`, `local_app_data`. A write a process inside that app's container
/// makes below `root` lands here.
pub(super) fn private_root(local_app_data: &Path, family: &str, root: DataRoot) -> PathBuf {
    local_app_data
        .join(PACKAGES_DIRECTORY)
        .join(family)
        .join(LOCAL_CACHE)
        .join(root.folder())
}

/// Whether `family` is a package family name of the package called `name`: that name, ignoring
/// case, an underscore and a publisher id of letters and digits.
pub(super) fn is_family_of(family: &str, name: &str) -> bool {
    family
        .rsplit_once(PACKAGE_FIELD_SEPARATOR)
        .is_some_and(|(own, publisher)| {
            own.eq_ignore_ascii_case(name) && is_publisher_id(publisher)
        })
}

/// Whether `family` has the shape of a package family name: a name, an underscore and a publisher
/// id.
fn is_family(family: &str) -> bool {
    family
        .rsplit_once(PACKAGE_FIELD_SEPARATOR)
        .is_some_and(|(own, publisher)| !own.is_empty() && is_publisher_id(publisher))
}

fn is_publisher_id(publisher: &str) -> bool {
    !publisher.is_empty() && publisher.chars().all(|unit| unit.is_ascii_alphanumeric())
}

/// The package families installed for the person below `local_app_data`, in family name order,
/// from one listing of [`PACKAGES_DIRECTORY`] of at most [`MAX_PACKAGES`] entries. A folder that
/// cannot be read lists none.
pub(super) fn families(local_app_data: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(local_app_data.join(PACKAGES_DIRECTORY)) else {
        return Vec::new();
    };
    let mut families: Vec<String> = entries
        .take(MAX_PACKAGES)
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|family| is_family(family))
        .collect();
    families.sort();
    families
}

/// The person's two data roots on Windows.
#[derive(Clone, Debug)]
pub(super) struct DataRoots {
    /// `%APPDATA%`.
    pub roaming: PathBuf,
    /// `%LOCALAPPDATA%`, which also holds [`PACKAGES_DIRECTORY`].
    pub local: PathBuf,
}

impl DataRoots {
    /// The root `path` lies below and the path relative to it; none for a path below neither.
    fn split<'a>(&self, path: &'a Path) -> Option<(DataRoot, &'a Path)> {
        DATA_ROOTS.into_iter().find_map(|root| {
            let base = match root {
                DataRoot::Roaming => &self.roaming,
                DataRoot::Local => &self.local,
            };
            path.strip_prefix(base).ok().map(|rest| (root, rest))
        })
    }
}

/// One package installed for the person that keeps a private copy of at least one data root.
#[derive(Clone, Debug)]
struct PackageData {
    /// The package family name.
    family: String,
    /// The data roots the package keeps a private copy of.
    roots: Vec<DataRoot>,
}

/// The packages one look found: a single listing of [`PACKAGES_DIRECTORY`].
#[derive(Clone, Debug, Default)]
pub(super) struct Packages {
    roots: Option<DataRoots>,
    packages: Vec<PackageData>,
}

/// Whose place a copy of an agent's settings is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CopyPlace {
    /// A private copy in a Store package of the agent's own app, the copy that app reads first.
    OwnPackage,
    /// The plain path below the data root.
    Plain,
    /// A private copy in another Store package, such as an app Ghostlight once ran inside, whose
    /// view no other app reads.
    OtherPackage,
}

/// One place an agent's settings may be: the plain path or a package's private copy of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SettingsCopy {
    /// The configuration file in that place, which need not exist yet.
    pub file: PathBuf,
    /// Whose place it is.
    pub place: CopyPlace,
}

/// How early the agent reads a copy, lowest first. The agent's own app reads its private file
/// first; while it has none, the app's merged view shows the plain file, so a plain file comes
/// next. Then the own app's settings folder without a file, where its first write lands, then the
/// plain path, with or without its folder. Another package's copy comes last, whatever exists:
/// the agent started on its own never reads it, so it is never the one chosen.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Reading {
    /// The own app's private settings file.
    OwnFile,
    /// The plain settings file.
    PlainFile,
    /// The own app's private settings folder, without the file.
    OwnFolder,
    /// The plain settings folder, without the file.
    PlainFolder,
    /// The plain path, where neither the file nor its folder exists.
    PlainAbsent,
    /// Another package's private settings folder, with or without the file.
    OtherPackage,
}

impl Packages {
    /// No packages: every configuration is only its plain path. Linux, and Windows where the data
    /// roots are not known, look no further.
    pub(super) fn none() -> Self {
        Self::default()
    }

    /// The packages installed for the person below `roots.local` that keep a private copy of a
    /// data root, in family name order. A folder that cannot be read lists none.
    pub(super) fn listed(roots: DataRoots) -> Self {
        let packages = families(&roots.local)
            .into_iter()
            .filter_map(|family| {
                let kept: Vec<DataRoot> = DATA_ROOTS
                    .into_iter()
                    .filter(|root| private_root(&roots.local, &family, *root).is_dir())
                    .collect();
                (!kept.is_empty()).then_some(PackageData {
                    family,
                    roots: kept,
                })
            })
            .collect();
        Self {
            roots: Some(roots),
            packages,
        }
    }

    /// Every place the settings whose plain file is `plain` are found, in the order the agent
    /// reads them ([`Reading`]); the first is the file the agent reads.
    ///
    /// `resolve` turns the file a place would hold by default into the configuration file the
    /// agent reads there, such as the first of several file names that exists. The plain copy's
    /// default is `plain` itself, and a private copy's is the same file name in its settings
    /// folder. A package is the agent's own when its family is one of the names in `own`. Copies
    /// read at the same point come most recently written first (the file, or the folder while it
    /// has no file), then by family name. A plain path below neither root, or directly in one, has
    /// no settings folder of its own and gets no private candidates.
    pub(super) fn copies(
        &self,
        plain: &Path,
        own: &[&str],
        resolve: impl Fn(&Path) -> PathBuf,
    ) -> Vec<SettingsCopy> {
        let plain_copy = SettingsCopy {
            file: resolve(plain),
            place: CopyPlace::Plain,
        };
        let Some(roots) = &self.roots else {
            return vec![plain_copy];
        };
        let Some((root, folder, name)) = roots
            .split(plain)
            .and_then(|(root, rest)| Some((root, rest.parent()?, rest.file_name()?)))
            .filter(|(_, folder, _)| !folder.as_os_str().is_empty())
        else {
            return vec![plain_copy];
        };
        let plain_reading = if plain_copy.file.is_file() {
            Reading::PlainFile
        } else if plain_copy.file.parent().is_some_and(Path::is_dir) {
            Reading::PlainFolder
        } else {
            Reading::PlainAbsent
        };
        let mut found: Vec<(Reading, Reverse<Option<SystemTime>>, &str, SettingsCopy)> = self
            .packages
            .iter()
            .filter(|package| package.roots.contains(&root))
            .filter_map(|package| {
                let settings = private_root(&roots.local, &package.family, root).join(folder);
                settings.is_dir().then(|| {
                    let file = resolve(&settings.join(name));
                    let mine = own.iter().any(|name| is_family_of(&package.family, name));
                    let (reading, place) = match (mine, file.is_file()) {
                        (true, true) => (Reading::OwnFile, CopyPlace::OwnPackage),
                        (true, false) => (Reading::OwnFolder, CopyPlace::OwnPackage),
                        (false, _) => (Reading::OtherPackage, CopyPlace::OtherPackage),
                    };
                    let written = written(&file).or_else(|| written(&settings));
                    (
                        reading,
                        Reverse(written),
                        package.family.as_str(),
                        SettingsCopy { file, place },
                    )
                })
            })
            .collect();
        found.push((plain_reading, Reverse(None), "", plain_copy));
        found.sort_by_key(|(reading, written, family, _)| (*reading, *written, *family));
        found.into_iter().map(|(_, _, _, copy)| copy).collect()
    }
}

/// When `path` was last written; none when that cannot be read.
fn written(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

#[cfg(test)]
mod tests {
    use std::fs::FileTimes;
    use std::time::Duration;

    use super::*;
    use crate::scratch::Scratch;

    /// The Store Claude Desktop's package family, as the owner's computer has it.
    const CLAUDE_FAMILY: &str = "Claude_pzs8sxrjxfjjc";

    /// The settings file of Claude Desktop, relative to `%APPDATA%`.
    const CLAUDE_SETTINGS: &str = "Claude/claude_desktop_config.json";

    /// The family of an app Ghostlight once ran inside, whose private view kept a copy.
    const CODEX_FAMILY: &str = "OpenAI.Codex_2p2nqsd0c76g0";

    const OWN: CopyPlace = CopyPlace::OwnPackage;
    const PLAIN: CopyPlace = CopyPlace::Plain;
    const OTHER: CopyPlace = CopyPlace::OtherPackage;

    fn roots(scratch: &Scratch) -> DataRoots {
        DataRoots {
            roaming: scratch.join("AppData/Roaming"),
            local: scratch.join("AppData/Local"),
        }
    }

    /// The file a place holds by default, as the agent reads it: a host with one file name.
    fn as_is(file: &Path) -> PathBuf {
        file.to_path_buf()
    }

    fn files(copies: &[SettingsCopy]) -> Vec<(PathBuf, CopyPlace)> {
        copies
            .iter()
            .map(|copy| (copy.file.clone(), copy.place))
            .collect()
    }

    /// Make the folder `relative` below the private copy of `root` of `family`.
    fn package(roots: &DataRoots, family: &str, root: DataRoot, relative: &str) -> PathBuf {
        let folder = private_root(&roots.local, family, root).join(relative);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// Write `file` and make it `seconds` old.
    fn written_ago(file: &Path, seconds: u64) {
        fs::write(file, "{}").unwrap();
        let when = SystemTime::now() - Duration::from_secs(seconds);
        let file = fs::File::options().write(true).open(file).unwrap();
        file.set_times(FileTimes::new().set_modified(when)).unwrap();
    }

    #[test]
    fn a_roaming_path_maps_to_local_cache_roaming_and_a_local_one_to_local_cache_local() {
        let scratch = Scratch::new();
        let roots = roots(&scratch);
        let roaming = package(&roots, CLAUDE_FAMILY, DataRoot::Roaming, "Claude");
        let local = package(
            &roots,
            "Vendor.Tool_abc123",
            DataRoot::Local,
            "Tool/settings",
        );
        let packages = Packages::listed(roots.clone());

        let plain = roots.roaming.join(CLAUDE_SETTINGS);
        assert_eq!(
            files(&packages.copies(&plain, &[], as_is)),
            [
                (plain.clone(), PLAIN),
                (roaming.join("claude_desktop_config.json"), OTHER)
            ]
        );
        assert_eq!(
            roaming,
            roots
                .local
                .join("Packages")
                .join(CLAUDE_FAMILY)
                .join("LocalCache/Roaming/Claude")
        );
        let plain = roots.local.join("Tool/settings/mcp.json");
        assert_eq!(
            files(&packages.copies(&plain, &[], as_is)),
            [(plain.clone(), PLAIN), (local.join("mcp.json"), OTHER)]
        );
        assert_eq!(
            local,
            roots
                .local
                .join("Packages/Vendor.Tool_abc123/LocalCache/Local/Tool/settings")
        );
    }

    #[test]
    fn without_the_package_or_its_settings_folder_only_the_plain_path_remains() {
        let scratch = Scratch::new();
        let roots = roots(&scratch);
        let plain = roots.roaming.join(CLAUDE_SETTINGS);
        let only_plain = vec![(plain.clone(), PLAIN)];
        // No Packages folder at all.
        let packages = Packages::listed(roots.clone());
        assert_eq!(files(&packages.copies(&plain, &[], as_is)), only_plain);
        // A package with a private copy of the root, but not of the client's folder.
        package(&roots, CLAUDE_FAMILY, DataRoot::Roaming, "Other");
        // A folder that is no package family, holding the client's folder.
        package(&roots, "NotAFamily", DataRoot::Roaming, "Claude");
        // A package of the wrong root.
        package(&roots, "Vendor.Local_abc", DataRoot::Local, "Claude");
        let packages = Packages::listed(roots.clone());
        assert_eq!(files(&packages.copies(&plain, &[], as_is)), only_plain);
        // A path below neither root, and one directly in a root, have no private copy.
        let elsewhere = scratch.join("home/.claude.json");
        assert_eq!(
            files(&packages.copies(&elsewhere, &[], as_is)),
            [(elsewhere.clone(), PLAIN)]
        );
        let loose = roots.roaming.join("loose.json");
        assert_eq!(
            files(&packages.copies(&loose, &[], as_is)),
            [(loose.clone(), PLAIN)]
        );
        // No roots known: nothing is looked at.
        assert_eq!(
            files(&Packages::none().copies(&plain, &[], as_is)),
            only_plain
        );
    }

    #[test]
    fn the_agents_own_package_wins_and_no_other_package_does() {
        let scratch = Scratch::new();
        let roots = roots(&scratch);
        let own = package(&roots, CLAUDE_FAMILY, DataRoot::Roaming, "Claude");
        let newer = package(&roots, "Other.App_xyz789", DataRoot::Roaming, "Claude");
        let older = package(&roots, "Older.App_abc123", DataRoot::Roaming, "Claude");
        let name = "claude_desktop_config.json";
        written_ago(&newer.join(name), 0);
        written_ago(&older.join(name), 300);
        written_ago(&own.join(name), 600);
        let packages = Packages::listed(roots.clone());
        let plain = roots.roaming.join(CLAUDE_SETTINGS);

        assert_eq!(
            files(&packages.copies(&plain, &["Claude"], as_is)),
            [
                (own.join(name), OWN),
                (plain.clone(), PLAIN),
                (newer.join(name), OTHER),
                (older.join(name), OTHER)
            ],
            "the agent's own Store app wins, then the plain path, then the others, written last first"
        );
        assert_eq!(
            files(&packages.copies(&plain, &[], as_is)),
            [
                (plain.clone(), PLAIN),
                (newer.join(name), OTHER),
                (older.join(name), OTHER),
                (own.join(name), OTHER)
            ],
            "without its own app the plain path leads, however new another package's copy is"
        );
    }

    #[test]
    fn another_packages_copy_never_comes_before_the_plain_path() {
        // As on leo-desktop-02 on 2026-10-20: Visual Studio Code had never been started, so its
        // plain settings folder did not exist, and the Codex app's container held a `Code\User`
        // folder that something run inside it once wrote. Visual Studio Code started on its own
        // never reads that copy.
        let scratch = Scratch::new();
        let roots = roots(&scratch);
        let leftover = package(&roots, CODEX_FAMILY, DataRoot::Roaming, "Code/User");
        written_ago(&leftover.join("mcp.json"), 0);
        let plain = roots.roaming.join("Code/User/mcp.json");
        let copies =
            |roots: &DataRoots| files(&Packages::listed(roots.clone()).copies(&plain, &[], as_is));
        let plain_first = vec![(plain.clone(), PLAIN), (leftover.join("mcp.json"), OTHER)];
        // No plain settings at all.
        assert_eq!(copies(&roots), plain_first);
        // The plain settings folder, without its file.
        fs::create_dir_all(plain.parent().unwrap()).unwrap();
        assert_eq!(copies(&roots), plain_first);
        // The plain file, however old it is.
        written_ago(&plain, 600);
        assert_eq!(copies(&roots), plain_first);
    }

    #[test]
    fn while_the_own_copy_has_no_file_the_plain_file_it_shows_comes_first() {
        let scratch = Scratch::new();
        let roots = roots(&scratch);
        let own = package(&roots, CLAUDE_FAMILY, DataRoot::Roaming, "Claude");
        let name = "claude_desktop_config.json";
        let plain = roots.roaming.join(CLAUDE_SETTINGS);
        let copies = || files(&Packages::listed(roots.clone()).copies(&plain, &["Claude"], as_is));
        // A fresh Store install with no plain settings: its own folder is where it writes.
        assert_eq!(copies(), [(own.join(name), OWN), (plain.clone(), PLAIN)]);
        // A plain folder without its file changes nothing.
        fs::create_dir_all(plain.parent().unwrap()).unwrap();
        assert_eq!(copies(), [(own.join(name), OWN), (plain.clone(), PLAIN)]);
        // A plain file is what the app's merged view shows while its own copy has no file.
        written_ago(&plain, 600);
        assert_eq!(copies(), [(plain.clone(), PLAIN), (own.join(name), OWN)]);
        // Once the own copy holds the file, the app reads it first.
        written_ago(&own.join(name), 900);
        assert_eq!(copies(), [(own.join(name), OWN), (plain.clone(), PLAIN)]);
    }

    #[test]
    fn a_family_is_a_package_name_and_a_publisher_id() {
        assert!(is_family_of(CLAUDE_FAMILY, "Claude"));
        assert!(is_family_of("claude_pzs8sxrjxfjjc", "Claude"));
        assert!(!is_family_of("ClaudeBeta_pzs8sxrjxfjjc", "Claude"));
        assert!(!is_family_of("Claude_", "Claude"));
        assert!(!is_family_of("Claude_pzs-8", "Claude"));
        assert!(is_family("OpenAI.Codex_2p2nqsd0c76g0"));
        assert!(!is_family("NotAFamily"));
        assert!(!is_family("_abc"));
    }
}

//! The Windows Start-menu shortcut and the installed-apps entry for the installation in `bin`.
//!
//! Every route installs into `home/bin`. The shortcut names the shell, `bin/ghostlight-desktop.exe`
//! (ADR-0199 Decision 4): a person's launch is the window, it is a GUI image so nothing flashes a
//! console, and it starts the service when none runs. The shortcut carries the AppUserModelID
//! `Sylin.Ghostlight` in a property store block, which is what groups the window with a pin and
//! lets an unpackaged program show a toast. A payload that has no shell leaves the shortcut on the
//! service image, which does the same through `open`. The per-user Uninstall key keeps the service
//! image for its icon and its command. Both take the names an NSIS install used, so writing them
//! also replaces what an NSIS install left behind; an NSIS Uninstall key is removed whole first,
//! so none of its values survive (ADR-0197 Decisions 14 and 16). Uninstalling removes the two
//! entries again, each only when it is provably this installation's (ADR-0199 Decision 20): the
//! shortcut when it targets a program in this home's `bin` or carries the AppUserModelID, the key
//! when its command or location points into this home. Anything else is left and named.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

use super::layout::Layout;
use super::record::atomic_replace;

/// The name the shortcut and the installed-apps entry carry.
const PRODUCT_NAME: &str = "Ghostlight";

/// The AppUserModelID of the shortcut. It is the shell's own id (`SHELL_APP_ID` in the shell
/// crate), written again here because the service depends on no UI crate; a test keeps them equal.
pub const APP_USER_MODEL_ID: &str = "Sylin.Ghostlight";

/// The per-user Uninstall key, below `HKEY_CURRENT_USER`.
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Ghostlight";

/// The Start-menu programs folder below `APPDATA`.
const PROGRAMS_FOLDER: [&str; 4] = ["Microsoft", "Windows", "Start Menu", "Programs"];

/// File extension of a shell link.
const SHORTCUT_EXTENSION: &str = "lnk";

/// The command the installed-apps entry runs to remove Ghostlight's registrations.
const UNINSTALL_ARGUMENT: &str = "uninstall";

/// What the report calls the Start-menu shortcut.
pub const START_MENU_SHORTCUT: &str = "Start-menu shortcut";

/// What the report calls the Uninstall key.
pub const INSTALLED_APPS_ENTRY: &str = "installed-apps entry";

/// Uninstall-key value names.
mod value {
    pub const DISPLAY_NAME: &str = "DisplayName";
    pub const DISPLAY_VERSION: &str = "DisplayVersion";
    pub const DISPLAY_ICON: &str = "DisplayIcon";
    pub const INSTALL_LOCATION: &str = "InstallLocation";
    pub const UNINSTALL_STRING: &str = "UninstallString";
    pub const NO_MODIFY: &str = "NoModify";
    pub const NO_REPAIR: &str = "NoRepair";
}

/// What [`remove`] or [`plan_removal`] found for one entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryFate {
    /// There is no such entry.
    Absent,
    /// The entry is this installation's. [`remove`] removed it; [`plan_removal`] says it would.
    Removed,
    /// The entry exists and does not name this installation, so it was left as it is.
    Kept,
    /// The entry could not be read or removed.
    Failed(String),
}

/// What removing the Windows entries found, entry by entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsEntryRemoval {
    /// The Start-menu shortcut.
    pub shortcut: EntryFate,
    /// The per-user Uninstall key.
    pub uninstall_key: EntryFate,
}

/// What reconciling the Windows entries changed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WindowsEntryChange {
    /// The Start-menu shortcut was written.
    pub shortcut: bool,
    /// The Uninstall key was written.
    pub uninstall_key: bool,
    /// An NSIS install's Uninstall key was removed first.
    pub nsis_removed: bool,
}

/// What [`reconcile`] would change, without writing anything.
pub fn plan(layout: &Layout, version: &str) -> io::Result<WindowsEntryChange> {
    let executable = layout.service_executable();
    let mut change = WindowsEntryChange::default();
    if let Some(shortcut) = shortcut_path() {
        let target = shortcut_target(layout);
        let list = ghostlight_win_peer::item_id_list(&target)?;
        let bytes = shortcut_bytes(&list, &target, layout.home());
        change.shortcut = fs::read(&shortcut).ok().as_deref() != Some(bytes.as_slice());
    }
    let uninstall = uninstall_command(&executable);
    let current = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(UNINSTALL_KEY, KEY_READ);
    let recorded: Option<String> = current
        .as_ref()
        .ok()
        .and_then(|key| key.get_value(value::UNINSTALL_STRING).ok());
    change.nsis_removed = recorded
        .as_ref()
        .is_some_and(|recorded| *recorded != uninstall);
    let version_recorded: Option<String> = current
        .ok()
        .and_then(|key| key.get_value(value::DISPLAY_VERSION).ok());
    change.uninstall_key =
        change.nsis_removed || recorded.is_none() || version_recorded.as_deref() != Some(version);
    Ok(change)
}

/// Point the Start-menu shortcut at the shell and the Uninstall key at the service in `layout`'s
/// `bin`.
///
/// Entries that already name them are left alone. A shortcut that names the service, or carries
/// no AppUserModelID, differs from the one written here and is rewritten.
pub fn reconcile(layout: &Layout, version: &str) -> io::Result<WindowsEntryChange> {
    let change = plan(layout, version)?;
    let executable = layout.service_executable();
    if change.shortcut {
        if let Some(shortcut) = shortcut_path() {
            let target = shortcut_target(layout);
            let list = ghostlight_win_peer::item_id_list(&target)?;
            atomic_replace(&shortcut, &shortcut_bytes(&list, &target, layout.home()))?;
        }
    }
    if change.nsis_removed {
        RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(UNINSTALL_KEY)?;
    }
    if change.uninstall_key {
        write_uninstall_key(
            layout,
            &executable,
            &uninstall_command(&executable),
            version,
        )?;
    }
    Ok(change)
}

/// What [`remove`] would do, without changing anything.
pub fn plan_removal(layout: &Layout) -> WindowsEntryRemoval {
    settle(layout, shortcut_path().as_deref(), &CurrentUserKey, false)
}

/// Remove the Start-menu shortcut and the Uninstall key when each is this installation's.
///
/// Ownership is read from the entry itself: the shortcut must target a program inside `layout`'s
/// `bin`, or carry the AppUserModelID when no target can be read from it, and the key's
/// `UninstallString` or `InstallLocation` must point into `layout`'s home. An entry that is not provably this installation's, such as one an
/// NSIS install or another home wrote, is left alone and reported as [`EntryFate::Kept`].
/// Removing what is already gone changes nothing, so a second call finds both
/// [`EntryFate::Absent`].
pub fn remove(layout: &Layout) -> WindowsEntryRemoval {
    settle(layout, shortcut_path().as_deref(), &CurrentUserKey, true)
}

/// Decide, and when `apply` is set carry out, the removal of both entries.
fn settle(
    layout: &Layout,
    shortcut: Option<&Path>,
    key: &dyn UninstallKey,
    apply: bool,
) -> WindowsEntryRemoval {
    WindowsEntryRemoval {
        shortcut: settle_shortcut(layout, shortcut, apply),
        uninstall_key: settle_key(layout, key, apply),
    }
}

/// The shortcut at `path` is removed when it targets a program in `bin`. A target it names
/// decides; only a shortcut with no readable target is judged by the product's AppUserModelID,
/// so another home's shortcut, which carries the same id, is kept.
fn settle_shortcut(layout: &Layout, path: Option<&Path>, apply: bool) -> EntryFate {
    let Some(path) = path else {
        return EntryFate::Absent;
    };
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return EntryFate::Absent,
        Err(error) => return EntryFate::Failed(error.to_string()),
    };
    let facts = read_shortcut(&bytes);
    let own = match facts.target.as_deref() {
        Some(target) => is_inside(target, &layout.bin()),
        None => facts.app_user_model_id.as_deref() == Some(APP_USER_MODEL_ID),
    };
    if !own {
        return EntryFate::Kept;
    }
    if !apply {
        return EntryFate::Removed;
    }
    match fs::remove_file(path) {
        Ok(()) => EntryFate::Removed,
        Err(error) if error.kind() == io::ErrorKind::NotFound => EntryFate::Absent,
        Err(error) => EntryFate::Failed(error.to_string()),
    }
}

/// The Uninstall key is removed when its command or its location points into the home.
fn settle_key(layout: &Layout, key: &dyn UninstallKey, apply: bool) -> EntryFate {
    if !key.exists() {
        return EntryFate::Absent;
    }
    let own = [value::UNINSTALL_STRING, value::INSTALL_LOCATION]
        .into_iter()
        .filter_map(|name| key.value(name))
        .any(|recorded| is_inside(&recorded, layout.home()));
    if !own {
        return EntryFate::Kept;
    }
    if !apply {
        return EntryFate::Removed;
    }
    match key.delete() {
        Ok(()) => EntryFate::Removed,
        Err(error) => EntryFate::Failed(error.to_string()),
    }
}

/// The per-user Uninstall key, behind a seam so a test needs no registry.
trait UninstallKey {
    /// Whether the key exists.
    fn exists(&self) -> bool;
    /// The string value `name`, when the key holds one.
    fn value(&self, name: &str) -> Option<String>;
    /// Delete the key with everything below it. A key that is already gone is not an error.
    fn delete(&self) -> io::Result<()>;
}

/// The Uninstall key of the current user.
struct CurrentUserKey;

impl CurrentUserKey {
    fn open() -> io::Result<RegKey> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(UNINSTALL_KEY, KEY_READ)
    }
}

impl UninstallKey for CurrentUserKey {
    fn exists(&self) -> bool {
        Self::open().is_ok()
    }

    fn value(&self, name: &str) -> Option<String> {
        Self::open().ok()?.get_value(name).ok()
    }

    fn delete(&self) -> io::Result<()> {
        match RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(UNINSTALL_KEY) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

/// Whether `candidate`, a path or a command line that starts with one, is `directory` or below
/// it. Case, slash direction, a leading quote and the `\\?\` prefix do not matter.
fn is_inside(candidate: &str, directory: &Path) -> bool {
    let directory = comparable(&directory.to_string_lossy());
    let directory = directory.trim_end_matches('\\');
    let candidate = comparable(candidate);
    candidate == directory
        || candidate
            .strip_prefix(directory)
            .is_some_and(|rest| rest.starts_with('\\'))
}

/// `path` lower-case, backslash-separated, without a leading quote or `\\?\` prefix.
fn comparable(path: &str) -> String {
    let path = path
        .trim_start_matches('"')
        .replace('/', "\\")
        .to_lowercase();
    match path.strip_prefix(r"\\?\") {
        Some(rest) => rest.to_owned(),
        None => path,
    }
}

/// What a shortcut says about itself.
#[derive(Debug, Default, Eq, PartialEq)]
struct ShortcutFacts {
    /// The program it starts, from the link's local path.
    target: Option<String>,
    /// Its `System.AppUserModel.ID`.
    app_user_model_id: Option<String>,
}

/// Read the target and the AppUserModelID of a shell link. A link that is cut short or is not a
/// shell link yields what was read before the fault.
fn read_shortcut(bytes: &[u8]) -> ShortcutFacts {
    let mut facts = ShortcutFacts::default();
    let _ = scan_shortcut(bytes, &mut facts);
    facts
}

/// Walk the header, the target list, LinkInfo, the string data and the extra data blocks.
fn scan_shortcut(bytes: &[u8], facts: &mut ShortcutFacts) -> Option<()> {
    if u32_at(bytes, 0)? != link::HEADER_SIZE || bytes.get(4..20)? != link::CLASS_ID {
        return None;
    }
    let flags = u32_at(bytes, 20)?;
    let mut at = link::HEADER_SIZE as usize;
    if flags & link::HAS_LINK_TARGET_ID_LIST != 0 {
        at += 2 + usize::from(u16_at(bytes, at)?);
    }
    if flags & link::HAS_LINK_INFO != 0 {
        let size = u32_at(bytes, at)? as usize;
        facts.target = link_info_target(bytes.get(at..at.checked_add(size)?)?);
        at += size;
    }
    let width = if flags & link::IS_UNICODE != 0 { 2 } else { 1 };
    for present in [
        link::HAS_NAME,
        link::HAS_RELATIVE_PATH,
        link::HAS_WORKING_DIR,
        link::HAS_ARGUMENTS,
        link::HAS_ICON_LOCATION,
    ] {
        if flags & present != 0 {
            at += 2 + usize::from(u16_at(bytes, at)?) * width;
        }
    }
    facts.app_user_model_id = app_user_model_id_in(bytes.get(at..)?);
    Some(())
}

/// The local path of a LinkInfo structure, base path and common suffix joined.
fn link_info_target(info: &[u8]) -> Option<String> {
    if u32_at(info, 8)? & link::VOLUME_ID_AND_LOCAL_BASE_PATH == 0 {
        return None;
    }
    let unicode = u32_at(info, 4)? >= link::LINK_INFO_HEADER_SIZE;
    let text = |offset: u32| -> Option<String> {
        if offset == 0 {
            return Some(String::new());
        }
        let tail = info.get(offset as usize..)?;
        Some(if unicode {
            decode_utf16(tail)
        } else {
            tail.iter()
                .take_while(|byte| **byte != 0)
                .map(|byte| char::from(*byte))
                .collect()
        })
    };
    let (base, suffix) = if unicode {
        (u32_at(info, 28)?, u32_at(info, 32)?)
    } else {
        (u32_at(info, 16)?, u32_at(info, 24)?)
    };
    Some(format!(
        "{}{}",
        text(base)?,
        text(suffix).unwrap_or_default()
    ))
}

/// The AppUserModelID in the property store block of the extra data, when there is one.
fn app_user_model_id_in(extra: &[u8]) -> Option<String> {
    let mut at = 0;
    loop {
        let size = u32_at(extra, at)? as usize;
        if size < 4 {
            return None;
        }
        let block = extra.get(at..at.checked_add(size)?)?;
        if u32_at(block, 4) == Some(link::PROPERTY_STORE_SIGNATURE) {
            return application_id_in_storages(block.get(8..)?);
        }
        at += size;
    }
}

/// Look through the serialized property storages for the application format's id property.
fn application_id_in_storages(storages: &[u8]) -> Option<String> {
    let mut at = 0;
    loop {
        let size = u32_at(storages, at)? as usize;
        if size == 0 {
            return None;
        }
        let storage = storages.get(at..at.checked_add(size)?)?;
        if storage.get(8..24) == Some(link::APPLICATION_FORMAT_ID.as_slice()) {
            let mut value_at = 24;
            loop {
                let value_size = u32_at(storage, value_at)? as usize;
                if value_size == 0 {
                    break;
                }
                let value = storage.get(value_at..value_at.checked_add(value_size)?)?;
                if u32_at(value, 4) == Some(link::APP_USER_MODEL_ID_PROPERTY)
                    && u16_at(value, 9) == Some(link::VT_LPWSTR)
                {
                    let characters = u32_at(value, 13)? as usize;
                    let end = 17usize.checked_add(characters.checked_mul(2)?)?;
                    return Some(decode_utf16(value.get(17..end)?));
                }
                value_at += value_size;
            }
        }
        at += size;
    }
}

/// The little-endian `u32` at `at`.
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(at..end)?.try_into().ok()?))
}

/// The little-endian `u16` at `at`.
fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let end = at.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(at..end)?.try_into().ok()?))
}

/// The UTF-16LE text in `bytes` up to the first terminator or the end.
fn decode_utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Write every value of the Uninstall key.
fn write_uninstall_key(
    layout: &Layout,
    executable: &Path,
    uninstall: &str,
    version: &str,
) -> io::Result<()> {
    let (key, _) =
        RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(UNINSTALL_KEY, KEY_WRITE)?;
    key.set_value(value::DISPLAY_NAME, &PRODUCT_NAME)?;
    key.set_value(value::DISPLAY_VERSION, &version)?;
    key.set_value(
        value::DISPLAY_ICON,
        &executable.to_string_lossy().into_owned(),
    )?;
    key.set_value(
        value::INSTALL_LOCATION,
        &layout.bin().to_string_lossy().into_owned(),
    )?;
    key.set_value(value::UNINSTALL_STRING, &uninstall)?;
    key.set_value(value::NO_MODIFY, &1u32)?;
    key.set_value(value::NO_REPAIR, &1u32)
}

/// The Uninstall key's command: the installed service removing Ghostlight's registrations.
fn uninstall_command(executable: &Path) -> String {
    format!("\"{}\" {UNINSTALL_ARGUMENT}", executable.display())
}

/// What the Start-menu shortcut names: the shell, or the service image when `bin` holds no shell.
fn shortcut_target(layout: &Layout) -> PathBuf {
    let shell = layout.desktop_executable();
    if shell.is_file() {
        shell
    } else {
        layout.service_executable()
    }
}

/// The Start-menu shortcut path, when this user has a roaming profile.
fn shortcut_path() -> Option<PathBuf> {
    let app_data = env::var_os("APPDATA").map(PathBuf::from)?;
    let folder = PROGRAMS_FOLDER
        .iter()
        .fold(app_data, |path, part| path.join(part));
    Some(folder.join(format!("{PRODUCT_NAME}.{SHORTCUT_EXTENSION}")))
}

/// Shell-link format constants (MS-SHLLINK).
mod link {
    /// Size of the shell-link header.
    pub const HEADER_SIZE: u32 = 0x4C;
    /// The shell-link class identifier, 00021401-0000-0000-C000-000000000046, as stored.
    pub const CLASS_ID: [u8; 16] = [
        0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x46,
    ];
    /// The link carries the target's item identifier list.
    pub const HAS_LINK_TARGET_ID_LIST: u32 = 0x0000_0001;
    /// The link carries a LinkInfo structure.
    pub const HAS_LINK_INFO: u32 = 0x0000_0002;
    /// The link carries a name.
    pub const HAS_NAME: u32 = 0x0000_0004;
    /// The link carries a relative path.
    pub const HAS_RELATIVE_PATH: u32 = 0x0000_0008;
    /// The link carries a working directory.
    pub const HAS_WORKING_DIR: u32 = 0x0000_0010;
    /// The link carries command-line arguments.
    pub const HAS_ARGUMENTS: u32 = 0x0000_0020;
    /// The link carries an icon location.
    pub const HAS_ICON_LOCATION: u32 = 0x0000_0040;
    /// Strings are UTF-16.
    pub const IS_UNICODE: u32 = 0x0000_0080;
    /// The target is an ordinary file.
    pub const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
    /// Open the target in a normal window.
    pub const SHOW_NORMAL: u32 = 1;
    /// LinkInfo header size with the Unicode offsets.
    pub const LINK_INFO_HEADER_SIZE: u32 = 0x24;
    /// LinkInfo carries a volume and a local base path.
    pub const VOLUME_ID_AND_LOCAL_BASE_PATH: u32 = 0x0000_0001;
    /// A fixed drive.
    pub const DRIVE_FIXED: u32 = 3;
    /// Offset of the empty volume label inside the VolumeID structure.
    pub const VOLUME_LABEL_OFFSET: u32 = 0x10;
    /// Signature of the property store block of the extra data.
    pub const PROPERTY_STORE_SIGNATURE: u32 = 0xA000_0009;
    /// Version of a serialized property storage, "1SPS".
    pub const PROPERTY_STORAGE_VERSION: u32 = 0x5350_5331;
    /// The format id of the shell's application properties,
    /// 9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3, as stored.
    pub const APPLICATION_FORMAT_ID: [u8; 16] = [
        0x55, 0x28, 0x4C, 0x9F, 0x79, 0x9F, 0x39, 0x4B, 0xA8, 0xD0, 0xE1, 0xD4, 0x2D, 0xE1, 0xD5,
        0xF3,
    ];
    /// The property id of `System.AppUserModel.ID` in that format.
    pub const APP_USER_MODEL_ID_PROPERTY: u32 = 5;
    /// The property type of a Unicode string, `VT_LPWSTR`.
    pub const VT_LPWSTR: u16 = 0x001F;
}

/// A shell link to `target` that starts in `working_directory`. `id_list` is the shell's item
/// identifier list for `target`, terminator included.
fn shortcut_bytes(id_list: &[u8], target: &Path, working_directory: &Path) -> Vec<u8> {
    let target = target.to_string_lossy();
    let mut bytes = Vec::new();
    bytes.extend(link::HEADER_SIZE.to_le_bytes());
    bytes.extend(link::CLASS_ID);
    let flags = link::HAS_LINK_TARGET_ID_LIST
        | link::HAS_LINK_INFO
        | link::HAS_WORKING_DIR
        | link::IS_UNICODE;
    bytes.extend(flags.to_le_bytes());
    bytes.extend(link::FILE_ATTRIBUTE_NORMAL.to_le_bytes());
    bytes.extend([0u8; 24]); // creation, access and write times
    bytes.extend(0u32.to_le_bytes()); // file size
    bytes.extend(0i32.to_le_bytes()); // icon index
    bytes.extend(link::SHOW_NORMAL.to_le_bytes());
    bytes.extend([0u8; 12]); // hot key and reserved fields
    let id_list_size = u16::try_from(id_list.len()).unwrap_or(u16::MAX);
    bytes.extend(id_list_size.to_le_bytes());
    bytes.extend(&id_list[..usize::from(id_list_size)]);

    let mut volume = Vec::new();
    volume.extend(0u32.to_le_bytes()); // size, filled below
    volume.extend(link::DRIVE_FIXED.to_le_bytes());
    volume.extend(0u32.to_le_bytes()); // serial number
    volume.extend(link::VOLUME_LABEL_OFFSET.to_le_bytes());
    volume.push(0); // empty label
    let volume_size = u32::try_from(volume.len()).expect("a fixed-size structure");
    volume[..4].copy_from_slice(&volume_size.to_le_bytes());

    let ansi: Vec<u8> = target
        .chars()
        .map(|character| {
            if character.is_ascii() {
                character as u8
            } else {
                b'?'
            }
        })
        .chain([0])
        .collect();
    let unicode: Vec<u8> = utf16(&target).chain([0, 0]).collect();
    let header = link::LINK_INFO_HEADER_SIZE as usize;
    let volume_offset = header;
    let base_offset = volume_offset + volume.len();
    let suffix_offset = base_offset + ansi.len();
    let unicode_base_offset = suffix_offset + 1;
    let unicode_suffix_offset = unicode_base_offset + unicode.len();
    let total = unicode_suffix_offset + 2;
    let mut info = Vec::with_capacity(total);
    for field in [
        total,
        header,
        link::VOLUME_ID_AND_LOCAL_BASE_PATH as usize,
        volume_offset,
        base_offset,
        0, // no network location
        suffix_offset,
        unicode_base_offset,
        unicode_suffix_offset,
    ] {
        let field = u32::try_from(field).expect("a path shorter than 4 GiB");
        info.extend(field.to_le_bytes());
    }
    info.extend(volume);
    info.extend(ansi);
    info.push(0); // empty common path suffix
    info.extend(unicode);
    info.extend([0, 0]); // empty Unicode common path suffix
    bytes.extend(info);

    let directory = working_directory.to_string_lossy();
    let characters = u16::try_from(directory.encode_utf16().count()).unwrap_or(u16::MAX);
    bytes.extend(characters.to_le_bytes());
    bytes.extend(utf16(&directory).take(usize::from(characters) * 2));
    bytes.extend(app_user_model_id_block(APP_USER_MODEL_ID));
    bytes.extend(0u32.to_le_bytes()); // the terminal block
    bytes
}

/// The property store block of the extra data that holds `System.AppUserModel.ID` = `id`, as one
/// serialized property storage (MS-SHLLINK 2.5.7, MS-PROPSTORE 2.2).
fn app_user_model_id_block(id: &str) -> Vec<u8> {
    let mut text: Vec<u8> = utf16(id).chain([0, 0]).collect();
    let characters = u32::try_from(text.len() / 2).expect("an id shorter than 4 GiB");
    while !text.len().is_multiple_of(4) {
        text.push(0);
    }
    let mut value = Vec::new();
    value.extend(0u32.to_le_bytes()); // size, filled below
    value.extend(link::APP_USER_MODEL_ID_PROPERTY.to_le_bytes());
    value.push(0); // reserved
    value.extend(link::VT_LPWSTR.to_le_bytes());
    value.extend(0u16.to_le_bytes()); // padding
    value.extend(characters.to_le_bytes());
    value.extend(text);
    let value_size = u32::try_from(value.len()).expect("a short property");
    value[..4].copy_from_slice(&value_size.to_le_bytes());

    let mut storage = Vec::new();
    storage.extend(0u32.to_le_bytes()); // size, filled below
    storage.extend(link::PROPERTY_STORAGE_VERSION.to_le_bytes());
    storage.extend(link::APPLICATION_FORMAT_ID);
    storage.extend(value);
    storage.extend(0u32.to_le_bytes()); // the end of this storage's properties
    let storage_size = u32::try_from(storage.len()).expect("a short storage");
    storage[..4].copy_from_slice(&storage_size.to_le_bytes());

    let mut block = Vec::new();
    let block_size = u32::try_from(8 + storage.len() + 4).expect("a short block");
    block.extend(block_size.to_le_bytes());
    block.extend(link::PROPERTY_STORE_SIGNATURE.to_le_bytes());
    block.extend(storage);
    block.extend(0u32.to_le_bytes()); // the end of the property store
    block
}

/// The UTF-16LE bytes of `text`, without a terminator.
fn utf16(text: &str) -> impl Iterator<Item = u8> + '_ {
    text.encode_utf16().flat_map(u16::to_le_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u32(bytes: &[u8], at: usize) -> usize {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
    }

    #[test]
    fn a_shortcut_names_its_target_and_working_directory() {
        let target = Path::new("C:\\Users\\Zoe \u{e9}\\.ghostlight\\bin\\ghostlight-service.exe");
        let home = Path::new("C:\\Users\\Zoe \u{e9}\\.ghostlight");
        let id_list = [4, 0, 1, 2, 0, 0];
        let bytes = shortcut_bytes(&id_list, target, home);
        assert_eq!(read_u32(&bytes, 0), link::HEADER_SIZE as usize);
        assert_eq!(&bytes[4..20], &link::CLASS_ID);
        let start = link::HEADER_SIZE as usize;
        assert_eq!(&bytes[start..start + 2], &[6, 0]);
        assert_eq!(&bytes[start + 2..start + 8], &id_list);
        let header = start + 8;
        assert_eq!(
            read_u32(&bytes, header + 4),
            link::LINK_INFO_HEADER_SIZE as usize
        );
        let info = &bytes[header..];
        let info_size = read_u32(info, 0);
        let unicode_base = read_u32(info, 28);
        let unicode_path: Vec<u16> = info[unicode_base..]
            .chunks(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|unit| *unit != 0)
            .collect();
        assert_eq!(
            String::from_utf16(&unicode_path).unwrap(),
            target.to_string_lossy()
        );
        let ansi_base = read_u32(info, 16);
        assert!(info[ansi_base..].starts_with(b"C:\\Users\\Zoe ?\\"));
        let strings = &bytes[header + info_size..];
        let characters = usize::from(u16::from_le_bytes([strings[0], strings[1]]));
        let directory: Vec<u16> = strings[2..2 + characters * 2]
            .chunks(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        assert_eq!(
            String::from_utf16(&directory).unwrap(),
            home.to_string_lossy()
        );
        // What follows the strings is the property store block and then the terminal block.
        let extra = &strings[2 + characters * 2..];
        assert_eq!(&extra[extra.len() - 4..], &[0, 0, 0, 0]);
        assert_eq!(
            read_u32(extra, 0),
            extra.len() - 4,
            "the block fills the extra data"
        );
    }

    /// The AppUserModelID a shortcut's property store block holds, read back from its bytes.
    fn app_user_model_id_of(shortcut: &[u8]) -> Option<String> {
        let signature = link::PROPERTY_STORE_SIGNATURE.to_le_bytes();
        let block = shortcut
            .windows(4)
            .rposition(|window| window == signature)?
            - 4;
        let size = read_u32(shortcut, block);
        let storage = block + 8;
        assert_eq!(
            read_u32(shortcut, storage + 4),
            link::PROPERTY_STORAGE_VERSION as usize
        );
        assert_eq!(
            &shortcut[storage + 8..storage + 24],
            &link::APPLICATION_FORMAT_ID
        );
        let value = storage + 24;
        assert_eq!(
            read_u32(shortcut, value + 4),
            link::APP_USER_MODEL_ID_PROPERTY as usize
        );
        assert_eq!(shortcut[value + 8], 0, "reserved");
        assert_eq!(
            u16::from_le_bytes([shortcut[value + 9], shortcut[value + 10]]),
            link::VT_LPWSTR
        );
        let characters = read_u32(shortcut, value + 13);
        let text: Vec<u16> = shortcut[value + 17..value + 17 + characters * 2]
            .chunks(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|unit| *unit != 0)
            .collect();
        // The storage ends where its one value ends, and the block where the storage list does.
        let value_size = read_u32(shortcut, value);
        assert_eq!(read_u32(shortcut, value + value_size), 0);
        assert_eq!(read_u32(shortcut, storage), value_size + 24 + 4);
        assert_eq!(block + size, shortcut.len() - 4);
        String::from_utf16(&text).ok()
    }

    #[test]
    fn a_shortcut_carries_the_app_user_model_id_and_it_reads_back() {
        let target = Path::new("C:\\h\\bin\\ghostlight-desktop.exe");
        let bytes = shortcut_bytes(&[4, 0, 1, 2, 0, 0], target, Path::new("C:\\h"));
        assert_eq!(
            app_user_model_id_of(&bytes).as_deref(),
            Some(APP_USER_MODEL_ID)
        );
    }

    #[test]
    fn a_shortcut_with_the_old_target_or_without_the_id_differs_from_the_one_written() {
        let home = Path::new("C:\\h");
        let list = [4, 0, 1, 2, 0, 0];
        let shell = shortcut_bytes(&list, Path::new("C:\\h\\bin\\ghostlight-desktop.exe"), home);
        let service = shortcut_bytes(&list, Path::new("C:\\h\\bin\\ghostlight-service.exe"), home);
        assert_ne!(shell, service, "an old shortcut is rewritten");
        // A shortcut made before this block existed ends its strings with the terminal block.
        let end = shell.len() - app_user_model_id_block(APP_USER_MODEL_ID).len() - 4;
        let mut without = shell[..end].to_vec();
        without.extend(0u32.to_le_bytes());
        assert_ne!(shell, without, "a shortcut without the id is rewritten");
        assert_eq!(
            app_user_model_id_of(&shell).as_deref(),
            Some(APP_USER_MODEL_ID)
        );
    }

    #[test]
    fn the_id_is_the_one_the_shell_names_itself() {
        assert_eq!(APP_USER_MODEL_ID, "Sylin.Ghostlight");
    }

    #[test]
    fn the_shortcut_names_the_shell_when_bin_holds_one_and_the_service_when_it_does_not() {
        let scratch = crate::install::scratch::Scratch::new();
        let layout = Layout::new(scratch.path());
        assert_eq!(shortcut_target(&layout), layout.service_executable());
        std::fs::create_dir_all(layout.bin()).unwrap();
        std::fs::write(layout.desktop_executable(), b"shell").unwrap();
        assert_eq!(shortcut_target(&layout), layout.desktop_executable());
    }

    /// A shortcut to `target` as an older install wrote it, without the AppUserModelID block.
    fn shortcut_without_id(target: &Path, home: &Path) -> Vec<u8> {
        let full = shortcut_bytes(&[4, 0, 1, 2, 0, 0], target, home);
        let end = full.len() - app_user_model_id_block(APP_USER_MODEL_ID).len() - 4;
        let mut bytes = full[..end].to_vec();
        bytes.extend(0u32.to_le_bytes());
        bytes
    }

    #[test]
    fn a_shortcut_reads_back_its_target_and_its_id() {
        let target = Path::new("C:\\Users\\Zoe \u{e9}\\.ghostlight\\bin\\ghostlight-desktop.exe");
        let home = Path::new("C:\\Users\\Zoe \u{e9}\\.ghostlight");
        let facts = read_shortcut(&shortcut_bytes(&[4, 0, 1, 2, 0, 0], target, home));
        assert_eq!(facts.target.as_deref(), Some(&*target.to_string_lossy()));
        assert_eq!(facts.app_user_model_id.as_deref(), Some(APP_USER_MODEL_ID));
        let older = read_shortcut(&shortcut_without_id(target, home));
        assert_eq!(older.target, facts.target);
        assert_eq!(older.app_user_model_id, None);
        assert_eq!(read_shortcut(b"not a shell link"), ShortcutFacts::default());
        let bytes = shortcut_bytes(&[4, 0, 1, 2, 0, 0], target, home);
        assert_eq!(
            read_shortcut(&bytes[..bytes.len() / 2]).app_user_model_id,
            None,
            "a cut-short link names no id"
        );
    }

    #[test]
    fn a_path_is_inside_a_directory_by_whole_components_and_without_regard_to_case() {
        let home = Path::new(r"C:\Users\Zoe\.ghostlight");
        assert!(is_inside(r"C:\Users\Zoe\.ghostlight", home));
        assert!(is_inside(r"c:/users/zoe/.GHOSTLIGHT/bin/x.exe", home));
        assert!(is_inside(
            r#""C:\Users\Zoe\.ghostlight\bin\ghostlight-service.exe" uninstall"#,
            home
        ));
        assert!(is_inside(r"\\?\C:\Users\Zoe\.ghostlight\bin", home));
        assert!(!is_inside(r"C:\Users\Zoe\.ghostlight-other\bin", home));
        assert!(!is_inside(
            r"C:\Program Files\Ghostlight\Ghostlight.exe",
            home
        ));
    }

    /// An in-memory Uninstall key.
    struct FakeKey {
        values: std::cell::RefCell<Option<Vec<(&'static str, String)>>>,
    }

    impl FakeKey {
        fn with(values: &[(&'static str, &str)]) -> Self {
            let values = values
                .iter()
                .map(|(name, text)| (*name, (*text).to_owned()))
                .collect();
            Self {
                values: std::cell::RefCell::new(Some(values)),
            }
        }

        fn absent() -> Self {
            Self {
                values: std::cell::RefCell::new(None),
            }
        }
    }

    impl UninstallKey for FakeKey {
        fn exists(&self) -> bool {
            self.values.borrow().is_some()
        }

        fn value(&self, name: &str) -> Option<String> {
            self.values
                .borrow()
                .as_ref()?
                .iter()
                .find(|(recorded, _)| *recorded == name)
                .map(|(_, text)| text.clone())
        }

        fn delete(&self) -> io::Result<()> {
            *self.values.borrow_mut() = None;
            Ok(())
        }
    }

    #[test]
    fn an_own_shortcut_and_an_own_key_are_removed_and_a_second_removal_changes_nothing() {
        let scratch = crate::install::scratch::Scratch::new();
        let layout = Layout::new(scratch.path().join("home"));
        let shortcut = scratch.path().join("Ghostlight.lnk");
        let list = [4, 0, 1, 2, 0, 0];
        fs::write(
            &shortcut,
            shortcut_bytes(&list, &layout.desktop_executable(), layout.home()),
        )
        .unwrap();
        let uninstall = uninstall_command(&layout.service_executable());
        let key = FakeKey::with(&[(value::UNINSTALL_STRING, &uninstall)]);

        let planned = settle(&layout, Some(&shortcut), &key, false);
        assert_eq!(planned.shortcut, EntryFate::Removed);
        assert_eq!(planned.uninstall_key, EntryFate::Removed);
        assert!(shortcut.is_file() && key.exists(), "a plan changes nothing");

        let removed = settle(&layout, Some(&shortcut), &key, true);
        assert_eq!(removed.shortcut, EntryFate::Removed);
        assert_eq!(removed.uninstall_key, EntryFate::Removed);
        assert!(!shortcut.exists() && !key.exists());

        let again = settle(&layout, Some(&shortcut), &key, true);
        assert_eq!(again.shortcut, EntryFate::Absent);
        assert_eq!(again.uninstall_key, EntryFate::Absent);
        assert_eq!(
            settle(&layout, None, &FakeKey::absent(), true).shortcut,
            EntryFate::Absent,
            "no profile folder, no shortcut"
        );
    }

    /// `bytes`, a link [`shortcut_bytes`] wrote, with its LinkInfo saying it holds no local path,
    /// so no target can be read from it.
    fn without_readable_target(mut bytes: Vec<u8>) -> Vec<u8> {
        let header = link::HEADER_SIZE as usize;
        let info = header + 2 + usize::from(u16::from_le_bytes([bytes[header], bytes[header + 1]]));
        let flags_at = info + 8;
        let flags = u32::from_le_bytes(bytes[flags_at..flags_at + 4].try_into().unwrap());
        let cleared = flags & !link::VOLUME_ID_AND_LOCAL_BASE_PATH;
        bytes[flags_at..flags_at + 4].copy_from_slice(&cleared.to_le_bytes());
        bytes
    }

    #[test]
    fn a_shortcut_is_own_by_its_target_and_by_its_id_only_when_it_names_none() {
        let scratch = crate::install::scratch::Scratch::new();
        let layout = Layout::new(scratch.path().join("home"));
        let shortcut = scratch.path().join("Ghostlight.lnk");
        let list = [4, 0, 1, 2, 0, 0];
        let other_home = Path::new(r"C:\Other\.ghostlight\bin\ghostlight-desktop.exe");
        fs::write(
            &shortcut,
            shortcut_without_id(&layout.service_executable(), layout.home()),
        )
        .unwrap();
        assert_eq!(
            settle_shortcut(&layout, Some(&shortcut), true),
            EntryFate::Removed,
            "an older shortcut that names the service in this bin"
        );
        let foreign = shortcut_bytes(&list, other_home, layout.home());
        assert_eq!(
            read_shortcut(&foreign).app_user_model_id.as_deref(),
            Some(APP_USER_MODEL_ID)
        );
        fs::write(&shortcut, &foreign).unwrap();
        assert_eq!(
            settle_shortcut(&layout, Some(&shortcut), true),
            EntryFate::Kept,
            "another home's shortcut carries the same id, and its target decides"
        );
        let targetless = without_readable_target(foreign);
        assert_eq!(read_shortcut(&targetless).target, None);
        fs::write(&shortcut, targetless).unwrap();
        assert_eq!(
            settle_shortcut(&layout, Some(&shortcut), true),
            EntryFate::Removed,
            "with no readable target, the AppUserModelID alone"
        );
    }

    #[test]
    fn a_shortcut_that_names_another_program_and_a_key_that_names_another_home_are_kept() {
        let scratch = crate::install::scratch::Scratch::new();
        let layout = Layout::new(scratch.path().join("home"));
        let shortcut = scratch.path().join("Ghostlight.lnk");
        let elsewhere = Path::new(r"C:\Program Files\Ghostlight\Ghostlight.exe");
        fs::write(
            &shortcut,
            shortcut_without_id(elsewhere, Path::new(r"C:\x")),
        )
        .unwrap();
        let before = fs::read(&shortcut).unwrap();
        let other_home = r#""C:\Other\.ghostlight\bin\ghostlight-service.exe" uninstall"#;
        let key = FakeKey::with(&[
            (value::UNINSTALL_STRING, other_home),
            (value::INSTALL_LOCATION, r"C:\Other\.ghostlight\bin"),
        ]);

        let removal = settle(&layout, Some(&shortcut), &key, true);

        assert_eq!(removal.shortcut, EntryFate::Kept);
        assert_eq!(removal.uninstall_key, EntryFate::Kept);
        assert_eq!(fs::read(&shortcut).unwrap(), before);
        assert!(key.exists());
        fs::write(&shortcut, b"garbage").unwrap();
        assert_eq!(
            settle_shortcut(&layout, Some(&shortcut), true),
            EntryFate::Kept,
            "a file that is not a readable shell link is not provably Ghostlight's"
        );
        assert!(
            settle_key(&layout, &FakeKey::with(&[]), true) == EntryFate::Kept,
            "a key that names nothing is not provably Ghostlight's"
        );
    }

    #[test]
    fn a_key_is_own_by_its_install_location_alone() {
        let scratch = crate::install::scratch::Scratch::new();
        let layout = Layout::new(scratch.path().join("home"));
        let location = layout.bin().to_string_lossy().into_owned();
        let key = FakeKey::with(&[(value::INSTALL_LOCATION, &location)]);
        assert_eq!(settle_key(&layout, &key, true), EntryFate::Removed);
        assert!(!key.exists());
    }

    #[test]
    fn the_uninstall_command_runs_the_installed_service() {
        assert_eq!(
            uninstall_command(Path::new(r"C:\h\bin\ghostlight-service.exe")),
            r#""C:\h\bin\ghostlight-service.exe" uninstall"#
        );
    }
}

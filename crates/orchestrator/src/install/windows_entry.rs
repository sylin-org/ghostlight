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
//! so none of its values survive (ADR-0197 Decisions 14 and 16).

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
    /// The link carries a working directory.
    pub const HAS_WORKING_DIR: u32 = 0x0000_0010;
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
        let native = include_str!("../../../desktop/src/native.rs");
        let declared = native
            .lines()
            .find_map(|line| {
                line.strip_prefix("pub const SHELL_APP_ID: &str = \"")
                    .and_then(|rest| rest.strip_suffix("\";"))
            })
            .expect("the shell declares its AppUserModelID");
        assert_eq!(APP_USER_MODEL_ID, declared);
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

    #[test]
    fn the_uninstall_command_runs_the_installed_service() {
        assert_eq!(
            uninstall_command(Path::new(r"C:\h\bin\ghostlight-service.exe")),
            r#""C:\h\bin\ghostlight-service.exe" uninstall"#
        );
    }
}

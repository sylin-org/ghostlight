//! A Windows program's own icon, read from its icon resource as a `.ico` file.
//!
//! The program is loaded as a data file, never run: Windows maps its resource section and nothing
//! of its code. The first icon group (`RT_GROUP_ICON`) names the program's icon in several sizes;
//! the largest few of those images (`RT_ICON`) are copied out and written as a standard `.ico`
//! file in memory, with no drawing and no conversion.

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

type Module = *mut c_void;

/// The callback `EnumResourceNamesW` calls once per resource name.
type EnumNames = unsafe extern "system" fn(Module, *const u16, *const u16, isize) -> i32;

/// Map the file for reading its data only: no code runs and no dependency loads.
const LOAD_LIBRARY_AS_DATAFILE: u32 = 0x0000_0002;

/// Map the file as an image, so its resources can be found through the image's sections.
const LOAD_LIBRARY_AS_IMAGE_RESOURCE: u32 = 0x0000_0020;

/// The resource type of one icon image.
const RT_ICON: usize = 3;

/// The resource type of an icon group, which names one icon's images in each size.
const RT_GROUP_ICON: usize = 14;

/// A resource name below this value is a numeric id carried in the pointer itself.
const INTEGER_RESOURCE_LIMIT: usize = 0x1_0000;

/// The longest string resource name copied, in UTF-16 units.
const MAX_NAME_UNITS: usize = 256;

/// The most images kept in the icon file: the largest sizes the program has.
const MAX_IMAGES: usize = 4;

/// The `ICONDIR` header of an icon file and of an icon group: reserved, type, count.
const DIRECTORY_BYTES: usize = 6;

/// One `GRPICONDIRENTRY` of an icon group.
const GROUP_ENTRY_BYTES: usize = 14;

/// One `ICONDIRENTRY` of an icon file.
const FILE_ENTRY_BYTES: usize = 16;

/// The type field of an icon, as opposed to a cursor.
const ICON_TYPE: u16 = 1;

/// The first bytes of an image stored as PNG.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The size field that opens a `BITMAPINFOHEADER`, how an image stored as a bitmap starts.
const BITMAP_HEADER_BYTES: u32 = 40;

/// The pixel size an icon entry's width byte of 0 stands for.
const LARGEST_ICON_PIXELS: u32 = 256;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(file: *const u16, reserved: *mut c_void, flags: u32) -> Module;
    fn FreeLibrary(module: Module) -> i32;
    fn EnumResourceNamesW(
        module: Module,
        kind: *const u16,
        callback: Option<EnumNames>,
        param: isize,
    ) -> i32;
    fn FindResourceW(module: Module, name: *const u16, kind: *const u16) -> *mut c_void;
    fn SizeofResource(module: Module, resource: *mut c_void) -> u32;
    fn LoadResource(module: Module, resource: *mut c_void) -> *mut c_void;
    fn LockResource(data: *mut c_void) -> *const c_void;
}

/// A program mapped as a data file; unmapped on drop.
struct DataFile(Module);

impl DataFile {
    fn load(path: &Path) -> io::Result<Self> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call, the reserved
        // argument is null as required, and the flags map the file as data only, so no code of
        // it runs. The guard frees every non-null module exactly once.
        let module = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
            )
        };
        if module.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(module))
    }

    /// The bytes of the resource `name` of type `kind`, copied out of the mapped file.
    fn resource(&self, name: *const u16, kind: usize) -> Option<Vec<u8>> {
        // SAFETY: the module is mapped while `self` lives; `name` is an integer resource id or a
        // NUL-terminated name that outlives the call, and `kind` is an integer resource type.
        let found = unsafe { FindResourceW(self.0, name, kind as *const u16) };
        if found.is_null() {
            return None;
        }
        // SAFETY: `found` is the resource handle FindResourceW just gave for this module.
        let size = unsafe { SizeofResource(self.0, found) } as usize;
        // SAFETY: as above; a loaded resource needs no freeing and lives as long as the module.
        let loaded = unsafe { LoadResource(self.0, found) };
        if loaded.is_null() || size == 0 {
            return None;
        }
        // SAFETY: `loaded` is the resource LoadResource just gave for this module.
        let data = unsafe { LockResource(loaded) };
        if data.is_null() {
            return None;
        }
        // SAFETY: LockResource points at the resource's `size` bytes, readable while the module
        // stays mapped, which `self` guarantees for the duration of the copy.
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };
        Some(bytes.to_vec())
    }

    /// The name of the first icon group, when the program has one.
    fn first_icon_group(&self) -> Option<ResourceName> {
        let mut first: Option<ResourceName> = None;
        // SAFETY: the module is mapped while `self` lives; the callback receives the address of
        // `first`, which outlives the enumeration and nothing else touches meanwhile. The
        // enumeration's own result is not needed: the callback stops it after the first name,
        // and a program with no icon group leaves `first` empty.
        unsafe {
            EnumResourceNamesW(
                self.0,
                RT_GROUP_ICON as *const u16,
                Some(take_first_name),
                std::ptr::addr_of_mut!(first) as isize,
            );
        }
        first
    }
}

impl Drop for DataFile {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns the module LoadLibraryExW mapped and frees it once.
        unsafe { FreeLibrary(self.0) };
    }
}

/// A resource's name: a numeric id or a string.
enum ResourceName {
    Id(u16),
    /// The name's UTF-16 units, NUL-terminated.
    Text(Vec<u16>),
}

impl ResourceName {
    /// The name as the resource functions take it.
    fn as_ptr(&self) -> *const u16 {
        match self {
            Self::Id(id) => usize::from(*id) as *const u16,
            Self::Text(units) => units.as_ptr(),
        }
    }
}

/// Keep the first resource name in the `Option<ResourceName>` at `param`, then stop.
extern "system" fn take_first_name(
    _module: Module,
    _kind: *const u16,
    name: *const u16,
    param: isize,
) -> i32 {
    let copied = if (name as usize) < INTEGER_RESOURCE_LIMIT {
        Some(ResourceName::Id(name as usize as u16))
    } else {
        let mut units = Vec::new();
        for offset in 0..MAX_NAME_UNITS {
            // SAFETY: a string resource name is a NUL-terminated UTF-16 string valid for this
            // callback; the walk stops at its terminator or after a bounded number of units.
            let unit = unsafe { *name.add(offset) };
            units.push(unit);
            if unit == 0 {
                break;
            }
        }
        (units.last() == Some(&0)).then_some(ResourceName::Text(units))
    };
    // SAFETY: `param` is the address of the `Option<ResourceName>` that `first_icon_group`
    // passes, alive and borrowed by nothing else while the enumeration runs.
    let first = unsafe { &mut *(param as *mut Option<ResourceName>) };
    *first = copied;
    0
}

/// One image an icon group names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GroupEntry {
    width: u8,
    height: u8,
    colors: u8,
    planes: u16,
    bit_count: u16,
    id: u16,
}

impl GroupEntry {
    /// The image's width in pixels; a width byte of 0 means 256.
    fn pixels(self) -> u32 {
        match self.width {
            0 => LARGEST_ICON_PIXELS,
            width => u32::from(width),
        }
    }
}

/// The icon of the program at `path` as a `.ico` file of at most `max_bytes`: the largest images
/// of its first icon group, up to [`MAX_IMAGES`] sizes. None when the program has no icon
/// resource or no image fits.
pub(crate) fn of_program(path: &Path, max_bytes: usize) -> io::Result<Option<Vec<u8>>> {
    let program = DataFile::load(path)?;
    let Some(group) = program.first_icon_group() else {
        return Ok(None);
    };
    let Some(directory) = program.resource(group.as_ptr(), RT_GROUP_ICON) else {
        return Ok(None);
    };
    let mut kept: Vec<(GroupEntry, Vec<u8>)> = Vec::new();
    let mut total = DIRECTORY_BYTES;
    for entry in largest_first(&group_entries(&directory)) {
        if kept.len() == MAX_IMAGES {
            break;
        }
        let Some(image) = program.resource(usize::from(entry.id) as *const u16, RT_ICON) else {
            continue;
        };
        if !is_icon_image(&image) || total + FILE_ENTRY_BYTES + image.len() > max_bytes {
            continue;
        }
        total += FILE_ENTRY_BYTES + image.len();
        kept.push((entry, image));
    }
    Ok((!kept.is_empty()).then(|| icon_file(&kept)))
}

/// The entries of an icon group's directory; none when it is not one.
fn group_entries(directory: &[u8]) -> Vec<GroupEntry> {
    let word = |at: usize| u16::from_le_bytes([directory[at], directory[at + 1]]);
    if directory.len() < DIRECTORY_BYTES || word(0) != 0 || word(2) != ICON_TYPE {
        return Vec::new();
    }
    let count = usize::from(word(4));
    if directory.len() < DIRECTORY_BYTES + count * GROUP_ENTRY_BYTES {
        return Vec::new();
    }
    (0..count)
        .map(|index| {
            let at = DIRECTORY_BYTES + index * GROUP_ENTRY_BYTES;
            GroupEntry {
                width: directory[at],
                height: directory[at + 1],
                colors: directory[at + 2],
                planes: word(at + 4),
                bit_count: word(at + 6),
                id: word(at + 12),
            }
        })
        .collect()
}

/// One entry per size, the deepest colour of each, largest size first.
fn largest_first(entries: &[GroupEntry]) -> Vec<GroupEntry> {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|entry| {
        (
            std::cmp::Reverse(entry.pixels()),
            std::cmp::Reverse(entry.bit_count),
        )
    });
    sorted.dedup_by_key(|entry| entry.pixels());
    sorted
}

/// Whether `image` is an icon image an icon file can hold: a PNG or a bitmap.
fn is_icon_image(image: &[u8]) -> bool {
    image.starts_with(PNG_SIGNATURE)
        || image
            .get(..4)
            .is_some_and(|size| size == BITMAP_HEADER_BYTES.to_le_bytes())
}

/// A `.ico` file of `images`, in order: the directory, one entry per image, then the images.
fn icon_file(images: &[(GroupEntry, Vec<u8>)]) -> Vec<u8> {
    let mut file = Vec::new();
    file.extend_from_slice(&0u16.to_le_bytes());
    file.extend_from_slice(&ICON_TYPE.to_le_bytes());
    file.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = DIRECTORY_BYTES + images.len() * FILE_ENTRY_BYTES;
    for (entry, image) in images {
        file.extend_from_slice(&[entry.width, entry.height, entry.colors, 0]);
        file.extend_from_slice(&entry.planes.to_le_bytes());
        file.extend_from_slice(&entry.bit_count.to_le_bytes());
        file.extend_from_slice(&(image.len() as u32).to_le_bytes());
        file.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += image.len();
    }
    for (_, image) in images {
        file.extend_from_slice(image);
    }
    file
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The images of an icon file: each entry's width and its image bytes.
    fn images_of(file: &[u8]) -> Vec<(u8, Vec<u8>)> {
        let word = |at: usize| u16::from_le_bytes([file[at], file[at + 1]]);
        let double = |at: usize| u32::from_le_bytes(file[at..at + 4].try_into().unwrap()) as usize;
        assert_eq!(word(0), 0, "reserved");
        assert_eq!(word(2), ICON_TYPE, "an icon, not a cursor");
        (0..usize::from(word(4)))
            .map(|index| {
                let at = DIRECTORY_BYTES + index * FILE_ENTRY_BYTES;
                let (size, offset) = (double(at + 8), double(at + 12));
                (file[at], file[offset..offset + size].to_vec())
            })
            .collect()
    }

    fn system_program(name: &str) -> std::path::PathBuf {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        Path::new(&root).join("System32").join(name)
    }

    /// Covers the icon-resource FFI (unsafe): notepad gives an icon file of its largest images, one per
    /// size, deepest colour and largest first, each placed after the directory; a small limit keeps
    /// only the images that fit, or none; and a program without an icon resource gives none while a
    /// file that is no program is an error.
    #[test]
    fn a_programs_icon_resource_becomes_an_icon_file_of_its_largest_images_that_fit() {
        {
            let icon = of_program(&system_program("notepad.exe"), 1024 * 1024)
                .unwrap()
                .expect("notepad has an icon");
            let images = images_of(&icon);
            assert!(!images.is_empty() && images.len() <= MAX_IMAGES);
            assert!(images.iter().all(|(_, image)| is_icon_image(image)));
            let widths: Vec<u32> = images
                .iter()
                .map(|(width, _)| if *width == 0 { 256 } else { u32::from(*width) })
                .collect();
            assert!(
                widths.windows(2).all(|pair| pair[0] > pair[1]),
                "{widths:?}"
            );
        }
        {
            let notepad = system_program("notepad.exe");
            let whole = of_program(&notepad, 1024 * 1024).unwrap().unwrap();
            let smaller = of_program(&notepad, whole.len() - 1).unwrap();
            assert!(smaller.is_none_or(|icon| icon.len() < whole.len()));
            assert_eq!(of_program(&notepad, DIRECTORY_BYTES).unwrap(), None);
        }
        {
            let this_test = std::env::current_exe().unwrap();
            assert_eq!(of_program(&this_test, 1024 * 1024).unwrap(), None);
            let scratch = crate::scratch::Scratch::named("program-icon");
            let text = scratch.join("not-a-program.exe");
            std::fs::write(&text, b"MZ not a program").unwrap();
            assert!(of_program(&text, 1024 * 1024).is_err());
        }
        {
            let entry = |width: u8, bit_count: u16, id: u16| GroupEntry {
                width,
                height: width,
                colors: 0,
                planes: 1,
                bit_count,
                id,
            };
            let mut directory = vec![0, 0, 1, 0, 4, 0];
            for item in [
                entry(16, 32, 1),
                entry(0, 32, 2),
                entry(48, 8, 3),
                entry(48, 32, 4),
            ] {
                directory.extend_from_slice(&[item.width, item.height, item.colors, 0]);
                directory.extend_from_slice(&item.planes.to_le_bytes());
                directory.extend_from_slice(&item.bit_count.to_le_bytes());
                directory.extend_from_slice(&100u32.to_le_bytes());
                directory.extend_from_slice(&item.id.to_le_bytes());
            }
            let ids: Vec<u16> = largest_first(&group_entries(&directory))
                .iter()
                .map(|entry| entry.id)
                .collect();
            assert_eq!(ids, [2, 4, 1]);
            assert!(group_entries(&directory[..20]).is_empty());
            assert!(group_entries(&[0, 0, 2, 0, 0, 0]).is_empty(), "a cursor");
        }
        {
            let png = [PNG_SIGNATURE, b"large"].concat();
            let bitmap = [&BITMAP_HEADER_BYTES.to_le_bytes()[..], b"small"].concat();
            let entry = |width: u8| GroupEntry {
                width,
                height: width,
                colors: 0,
                planes: 1,
                bit_count: 32,
                id: 1,
            };
            let file = icon_file(&[(entry(0), png.clone()), (entry(32), bitmap.clone())]);
            assert_eq!(images_of(&file), [(0, png), (32, bitmap)]);
            assert!(!is_icon_image(b"neither"));
        }
    }
}

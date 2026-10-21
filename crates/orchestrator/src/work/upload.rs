//! Files an upload names by path: read within the upload ceiling, and typed by extension.

use super::*;

pub(super) fn load_physical_files(
    paths: &[String],
) -> Result<(Vec<PhysicalFile>, u64), FileProblem> {
    use crate::language::UPLOAD_AGGREGATE_BYTES as MAX_UPLOAD_BYTES;
    let mut files = Vec::with_capacity(paths.len());
    let mut total = 0_u64;
    for requested in paths {
        let path = Path::new(requested);
        let mut file = File::open(path).map_err(|_| FileProblem::Unavailable)?;
        let before = file.metadata().map_err(|_| FileProblem::Unavailable)?;
        if !before.is_file() {
            return Err(FileProblem::NotAFile);
        }
        if before.len() > MAX_UPLOAD_BYTES {
            return Err(FileProblem::FileTooLarge);
        }
        total = total
            .checked_add(before.len())
            .ok_or(FileProblem::UploadTooLarge)?;
        if total > MAX_UPLOAD_BYTES {
            return Err(FileProblem::UploadTooLarge);
        }
        let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
        (&mut file)
            .take(MAX_UPLOAD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| FileProblem::ReadFailed)?;
        let after = file.metadata().map_err(|_| FileProblem::Changed)?;
        let modified_changed = before
            .modified()
            .ok()
            .zip(after.modified().ok())
            .is_some_and(|(left, right)| left != right);
        if u64::try_from(bytes.len()).ok() != Some(before.len())
            || after.len() != before.len()
            || modified_changed
        {
            return Err(FileProblem::Changed);
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .ok_or(FileProblem::InvalidName)?;
        files.push(PhysicalFile {
            name: bounded(name, crate::language::FILE_NAME_CHARS),
            media_type: media_type(path).into(),
            data: BASE64.encode(bytes),
            size: before.len(),
        });
    }
    Ok((files, total))
}

fn media_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "html" | "htm" => "text/html",
        _ => "application/octet-stream",
    }
}

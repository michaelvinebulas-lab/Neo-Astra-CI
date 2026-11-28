use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;

/// Extracts a downloaded package archive. The oracle tries `tar` first and
/// falls back to `unzip`; this mirrors that order in-process.
pub fn extract_archive(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let tar_error = extract_tar(bytes, destination).err();
    if tar_error.is_none() {
        return Ok(());
    }

    let zip_error = extract_zip(bytes, destination).err();
    if zip_error.is_none() {
        return Ok(());
    }

    Err(zip_error.or(tar_error).unwrap_or_default())
}

fn extract_tar(bytes: &[u8], destination: &Path) -> Result<(), String> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let decoder = GzDecoder::new(Cursor::new(bytes));
        let mut archive = tar::Archive::new(decoder);
        archive
            .unpack(destination)
            .map_err(|error| error.to_string())
    } else {
        let mut archive = tar::Archive::new(Cursor::new(bytes));
        archive
            .unpack(destination)
            .map_err(|error| error.to_string())
    }
}

fn extract_zip(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some(relative) = entry.enclosed_name() else {
            return Err("Zip archive contains an unsafe path.".to_owned());
        };
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut output = fs::File::create(&target).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut output).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// npm/PyPI/NuGet archives usually contain one top-level directory. The oracle
/// treats a single non-dot entry as the package root.
pub fn package_root(directory: &Path) -> PathBuf {
    let Ok(entries) = fs::read_dir(directory) else {
        return directory.to_path_buf();
    };
    let entries: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .collect();
    if entries.len() == 1 {
        let only = entries[0].path();
        if only.is_dir() {
            return only;
        }
    }
    directory.to_path_buf()
}

pub fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    #[test]
    fn extracts_gzipped_tar_archives() {
        let directory = tempfile::tempdir().unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            let mut header = tar::Header::new_gnu();
            header.set_size(5);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "package/index.txt", &b"hello"[..])
                .unwrap();
            builder.finish().unwrap();
        }
        let bytes = encoder.finish().unwrap();
        extract_archive(&bytes, directory.path()).unwrap();
        let root = package_root(directory.path());
        assert_eq!(root.file_name().unwrap(), "package");
        assert_eq!(fs::read_to_string(root.join("index.txt")).unwrap(), "hello");
    }

    #[test]
    fn extracts_zip_archives() {
        let directory = tempfile::tempdir().unwrap();
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            writer
                .start_file(
                    "package/index.txt",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(b"hello").unwrap();
            writer.finish().unwrap();
        }
        let bytes = buffer.into_inner();
        extract_archive(&bytes, directory.path()).unwrap();
        let root = package_root(directory.path());
        assert_eq!(fs::read_to_string(root.join("index.txt")).unwrap(), "hello");
    }
}

use crate::storage;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::{Dir, File, OpenOptions};
use scanner_types::error::Result;
use std::path::{Component, Path};

#[cfg(unix)]
pub(super) fn sync_directory(directory: &Dir) -> Result<()> {
    // Linux capability directories use O_PATH, which cannot be fsynced. Open
    // the pinned directory itself for reading without resolving an ambient path.
    let file = directory.open(".").map_err(storage)?;
    match file.sync_all() {
        Ok(()) => Ok(()),
        // Some network filesystems provide file sync but not directory sync.
        Err(error) if error.kind() == std::io::ErrorKind::Unsupported => Ok(()),
        Err(error) => Err(storage(error)),
    }
}

pub(super) fn create_directory(root: &Dir, name: &Path) -> std::io::Result<()> {
    let options = cap_std::fs::DirBuilder::new();
    #[cfg(unix)]
    let options = {
        use cap_std::fs::DirBuilderExt;
        let mut options = options;
        options.mode(0o700);
        options
    };
    root.create_dir_with(name, &options)
}

pub(super) fn components(path: &Path) -> Result<Vec<&std::ffi::OsStr>> {
    let parts: Option<Vec<_>> = path
        .components()
        .map(|c| match c {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    let parts =
        parts.ok_or_else(|| storage("Path must contain only relative normal components."))?;
    if parts.is_empty() || parts.len() > 32 {
        return Err(storage("Invalid path depth."));
    }
    Ok(parts)
}

pub(super) fn descend(root: &Dir, path: &Path) -> Result<Dir> {
    let mut directory = root.try_clone().map_err(storage)?;
    for name in components(path)? {
        directory = directory.open_dir_nofollow(name).map_err(storage)?;
    }
    Ok(directory)
}

pub(super) fn read_file(root: &Dir, path: &Path) -> Result<File> {
    let parts = components(path)?;
    let mut directory = root.try_clone().map_err(storage)?;
    for name in &parts[..parts.len() - 1] {
        directory = directory.open_dir_nofollow(name).map_err(storage)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let file = directory
        .open_with(parts[parts.len() - 1], &options)
        .map_err(storage)?;
    let metadata = file.metadata().map_err(storage)?;
    if !metadata.is_file() {
        return Err(storage("Expected a regular file."));
    }
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(storage("Hard-linked scan files are not accepted."));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, GetFileInformationByHandle,
        };
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: the owned file keeps its handle alive and info is writable.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(storage(std::io::Error::last_os_error()));
        }
        if info.nNumberOfLinks != 1 || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(storage(
                "Hard-linked or redirected scan files are not accepted.",
            ));
        }
    }
    Ok(file)
}

pub(super) fn create_file(root: &Dir, name: &str) -> Result<File> {
    if components(Path::new(name))?.len() != 1 {
        return Err(storage("Expected a single filename."));
    }
    let mut options = OpenOptions::new();
    options
        .write(true)
        .read(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    root.open_with(name, &options).map_err(storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hard_linked_images_are_refused_through_either_name() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("original"), b"original").unwrap();
        let dir = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority()).unwrap();
        assert!(read_file(&dir, Path::new("original")).is_ok());
        std::fs::hard_link(temp.path().join("original"), temp.path().join("alias")).unwrap();
        assert!(read_file(&dir, Path::new("original")).is_err());
        assert!(read_file(&dir, Path::new("alias")).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn capability_directories_can_be_synced_after_their_ambient_name_changes() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("before")).unwrap();
        let directory =
            Dir::open_ambient_dir(temp.path().join("before"), cap_std::ambient_authority())
                .unwrap();
        std::fs::rename(temp.path().join("before"), temp.path().join("after")).unwrap();
        sync_directory(&directory).unwrap();
        let child = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority())
            .unwrap()
            .open_dir_nofollow("after")
            .unwrap();
        sync_directory(&child).unwrap();
    }
}

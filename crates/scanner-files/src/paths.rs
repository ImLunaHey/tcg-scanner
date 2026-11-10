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
    let mut options = cap_std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use cap_std::fs::DirBuilderExt;
        options.mode(0o700);
    }
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
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

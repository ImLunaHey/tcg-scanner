use crate::error;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::{Dir, OpenOptions};
use scanner_types::error::Result;
use std::{
    fs::File,
    path::{Component, Path},
};

/// A pinned private local directory. Every component is opened without following
/// symlinks; callers use single filenames relative to the retained capability.
pub struct PrivateDirectory {
    dir: Dir,
}
impl PrivateDirectory {
    pub fn open(path: &Path, create: bool) -> Result<Self> {
        if !path.is_absolute() {
            return Err(error("Private state paths must be absolute."));
        }
        let root = path
            .ancestors()
            .last()
            .ok_or_else(|| error("Missing path root."))?;
        let mut dir = Dir::open_ambient_dir(root, cap_std::ambient_authority()).map_err(error)?;
        for component in path.components() {
            let name = match component {
                Component::Prefix(_) | Component::RootDir => continue,
                Component::Normal(name) => name,
                _ => {
                    return Err(error(
                        "Private state paths cannot traverse parent directories.",
                    ));
                }
            };
            dir = match dir.open_dir_nofollow(name) {
                Ok(dir) => dir,
                Err(e) if create && e.kind() == std::io::ErrorKind::NotFound => {
                    match create_directory(&dir, name) {
                        Ok(()) => (),
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                        Err(e) => return Err(error(e)),
                    }
                    dir.open_dir_nofollow(name).map_err(error)?
                }
                Err(e) => return Err(error(e)),
            };
        }
        validate(&dir.try_clone().map_err(error)?.into_std_file(), true)?;
        Ok(Self { dir })
    }
    pub fn open_file(
        &self,
        name: &str,
        writable: bool,
        create_new: bool,
        create: bool,
    ) -> Result<File> {
        if !writable && (create_new || create) {
            return Err(error("Creating private state requires writable access."));
        }
        if name.is_empty()
            || Path::new(name).components().count() != 1
            || !matches!(
                Path::new(name).components().next(),
                Some(Component::Normal(_))
            )
            || name.contains(['/', '\\', '\0', ':'])
        {
            return Err(error("Expected a single local state filename."));
        }
        let mut options = OpenOptions::new();
        #[cfg(windows)]
        if create_new || create {
            match crate::windows_create::create(&self.dir, std::ffi::OsStr::new(name), false) {
                Ok(file) => {
                    validate(&file, false)?;
                    return Ok(file);
                }
                Err(e)
                    if create && !create_new && e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(error(e)),
            }
        }
        options
            .read(true)
            .write(writable)
            .follow(FollowSymlinks::No)
            .nonblock(true);
        #[cfg(not(windows))]
        options.create_new(create_new).create(create);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = self
            .dir
            .open_with(name, &options)
            .map_err(error)?
            .into_std();
        validate(&file, false)?;
        Ok(file)
    }
    pub fn synchronize(&self) -> Result<()> {
        #[cfg(unix)]
        self.dir
            .open(".")
            .map_err(error)?
            .sync_all()
            .map_err(error)?;
        Ok(())
    }
    pub fn validate_existing_file(&self, name: &str) -> Result<bool> {
        if name.is_empty() || name.contains(['/', '\\', '\0', ':']) || matches!(name, "." | "..") {
            return Err(error("Expected a single local state filename."));
        }
        match self.dir.symlink_metadata(name) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(error(e)),
            Ok(_) => self.open_file(name, false, false, false).map(|_| true),
        }
    }
}
fn create_directory(dir: &Dir, name: &std::ffi::OsStr) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        crate::windows_create::create(dir, name, true).map(|_| ())
    }
    #[cfg(not(windows))]
    {
        let mut options = cap_std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use cap_std::fs::DirBuilderExt;
            options.mode(0o700);
        }
        dir.create_dir_with(name, &options)
    }
}
fn validate(file: &File, directory: bool) -> Result<()> {
    let metadata = file.metadata().map_err(error)?;
    if directory && !metadata.is_dir() || !directory && !metadata.is_file() {
        return Err(error("Expected a regular local state file or directory."));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid has no pointers or side effects.
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
            || !directory && metadata.nlink() != 1
        {
            return Err(error(
                "Local state must be private, owned by this user and not hard-linked.",
            ));
        }
    }
    #[cfg(windows)]
    crate::windows_acl::validate(file, directory)?;
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn private_children_and_inherited_sidecars_remain_private_and_reject_hard_links() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap().join("private");
        let directory = PrivateDirectory::open(&path, true).unwrap();
        drop(directory.open_file("state", true, true, false).unwrap());
        assert!(directory.open_file("state", true, true, false).is_err());
        // SQLite creates its own journals, with the token's default owner. The
        // protected parent ACL must be inherited even by elevated processes.
        std::fs::write(path.join("state-journal"), b"journal").unwrap();
        assert!(directory.validate_existing_file("state-journal").unwrap());
        std::fs::hard_link(path.join("state"), path.join("linked")).unwrap();
        assert!(directory.open_file("state", false, false, false).is_err());
    }
}

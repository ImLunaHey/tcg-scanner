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
                    #[cfg(unix)]
                    let options = {
                        use cap_std::fs::DirBuilderExt;
                        let mut options = cap_std::fs::DirBuilder::new();
                        options.mode(0o700);
                        options
                    };
                    #[cfg(not(unix))]
                    let options = cap_std::fs::DirBuilder::new();
                    match dir.create_dir_with(name, &options) {
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
        options
            .read(true)
            .write(writable)
            .create_new(create_new)
            .create(create)
            .follow(FollowSymlinks::No)
            .nonblock(true);
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
            .try_clone()
            .map_err(error)?
            .into_std_file()
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

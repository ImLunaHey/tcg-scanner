use super::{config, validate_name};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use scanner_types::error::Result;
use std::io::Read;
pub(super) fn bytes(root: &Dir, name: &str, limit: u64) -> Result<Vec<u8>> {
    validate_name(name)?;
    let mut dir = root.try_clone().map_err(config)?;
    let mut parts = name.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = dir.open_with(part, &options).map_err(config)?;
            let metadata = file.metadata().map_err(config)?;
            if !metadata.is_file() || metadata.len() > limit {
                return Err(config("Invalid distribution file type or size."));
            }
            let mut bytes = Vec::new();
            file.take(limit + 1)
                .read_to_end(&mut bytes)
                .map_err(config)?;
            if bytes.len() as u64 > limit {
                return Err(config("Distribution file exceeds limit."));
            }
            return Ok(bytes);
        }
        dir = dir.open_dir_nofollow(part).map_err(config)?;
    }
    Err(config("Missing distribution filename."))
}

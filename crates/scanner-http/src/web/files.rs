use super::invalid;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::{Dir, OpenOptions};
use scanner_types::error::Result;
use std::io::Read;

pub(super) fn read(root: &Dir, name: &str, limit: usize) -> Result<Vec<u8>> {
    let parts: Vec<_> = name.split('/').collect();
    let mut directory = root.try_clone().map_err(invalid)?;
    for part in &parts[..parts.len() - 1] {
        directory = directory.open_dir_nofollow(part).map_err(invalid)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let file = directory
        .open_with(parts[parts.len() - 1], &options)
        .map_err(invalid)?;
    let metadata = file.metadata().map_err(invalid)?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(invalid("Asset must be a bounded regular file."));
    }
    let mut bytes = vec![];
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() > limit {
        return Err(invalid("Asset exceeds its byte limit."));
    }
    Ok(bytes)
}

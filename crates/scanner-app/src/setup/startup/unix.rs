use crate::configuration::config;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use scanner_types::error::Result;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(super) fn set(binary: &Path, config_path: &Path, enable: bool) -> Result<()> {
    let binary = binary
        .to_str()
        .ok_or_else(|| config("Invalid executable path."))?;
    let source = config_path
        .to_str()
        .ok_or_else(|| config("Invalid configuration path."))?;
    if binary.contains(['\n', '\r']) || source.contains(['\n', '\r']) {
        return Err(config("Startup paths cannot contain newlines."));
    }
    let (path, contents) = document(binary, source)?;
    change(&path, contents.as_bytes(), enable)?;
    #[cfg(target_os = "linux")]
    {
        let wants = path.parent().unwrap().join("default.target.wants");
        std::fs::create_dir_all(&wants).map_err(config)?;
        let link = wants.join("scanner-rust.service");
        if enable {
            // Link creation is exclusive, so another installation is never replaced.
            if let Err(error) = std::os::unix::fs::symlink("../scanner-rust.service", &link) {
                let _ = change(&path, contents.as_bytes(), false);
                return Err(config(error));
            }
        } else if std::fs::read_link(&link).ok().as_deref()
            == Some(Path::new("../scanner-rust.service"))
        {
            std::fs::remove_file(&link).map_err(config)?;
        }
    }
    Ok(())
}
fn document(binary: &str, source: &str) -> Result<(PathBuf, String)> {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().ok_or_else(|| config("Home directory unavailable."))?;
        let output = home.join("Library/LaunchAgents/local.scanner.rust.plist");
        let contents = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>local.scanner.rust</string><key>ProgramArguments</key><array><string>{}</string><string>serve</string><string>--config</string><string>{}</string></array><key>RunAtLoad</key><true/><key>KeepAlive</key><false/><key>ProcessType</key><string>Interactive</string><key>ExitTimeOut</key><integer>60</integer></dict></plist>\n",
            super::xml(binary),
            super::xml(source)
        );
        Ok((output, contents))
    }
    #[cfg(target_os = "linux")]
    {
        let root =
            dirs::config_dir().ok_or_else(|| config("Configuration directory unavailable."))?;
        Ok((
            root.join("systemd/user/scanner-rust.service"),
            format!(
                "[Unit]\nDescription=Scanner Rust service\nAfter=network.target\n\n[Service]\nType=simple\nExecStart={} serve --config {}\nRestart=no\nTimeoutStopSec=60\nUMask=0077\n\n[Install]\nWantedBy=default.target\n",
                super::systemd(binary),
                super::systemd(source)
            ),
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (binary, source);
        Err(config("Startup is unsupported on this platform."))
    }
}
fn change(path: &Path, contents: &[u8], enable: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| config("Missing startup directory."))?;
    if enable {
        std::fs::create_dir_all(parent).map_err(config)?;
    }
    let directory = Dir::open_ambient_dir(parent, cap_std::ambient_authority()).map_err(config)?;
    let name = path
        .file_name()
        .ok_or_else(|| config("Missing startup filename."))?;
    let mut options = OpenOptions::new();
    options.follow(FollowSymlinks::No);
    if enable {
        use cap_std::fs::OpenOptionsExt;
        options.write(true).create_new(true).mode(0o600);
        let mut file = directory.open_with(name, &options).map_err(config)?;
        file.write_all(contents).map_err(config)?;
        file.sync_all().map_err(config)?;
    } else {
        options.read(true);
        let file = directory.open_with(name, &options).map_err(config)?;
        let mut actual = Vec::new();
        file.take(65537).read_to_end(&mut actual).map_err(config)?;
        if actual != contents {
            return Err(config(
                "Startup entry belongs to a different installation; retained.",
            ));
        }
        directory.remove_file(name).map_err(config)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_registration_never_replaces_or_removes_another_installation() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("startup/entry");
        change(&path, b"first", true).unwrap();
        assert!(change(&path, b"second", true).is_err());
        assert!(change(&path, b"second", false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        change(&path, b"first", false).unwrap();
        assert!(!path.exists());
    }
}

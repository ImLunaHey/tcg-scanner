use crate::{
    configuration::{Configuration, config},
    distribution::executable_name,
};
use scanner_types::error::Result;
use std::path::Path;
#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

pub(super) fn set(path: &Path, enable: bool) -> Result<()> {
    let path = path.canonicalize().map_err(config)?;
    Configuration::load(&path)?;
    let root = path
        .parent()
        .ok_or_else(|| config("Missing installation root."))?;
    let binary = root.join(executable_name("scanner-service"));
    if !binary.is_file() {
        return Err(config("Service executable is missing beside service.json."));
    }
    #[cfg(not(windows))]
    unix::set(&binary, &path, enable)?;
    #[cfg(windows)]
    windows::set(&binary, &path, enable)?;
    println!(
        "{} startup registration. Changes apply at the next login; running services were not changed.",
        if enable { "Created" } else { "Removed" }
    );
    Ok(())
}
#[cfg(any(target_os = "macos", test))]
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
#[cfg(any(target_os = "linux", test))]
fn systemd(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_arguments_escape_service_manager_syntax() {
        assert_eq!(xml("/a&b/<c>"), "/a&amp;b/&lt;c&gt;");
        assert_eq!(systemd("/a %x $VAR\""), "\"/a %%x $$VAR\\\"\"");
    }
}

//! Compatibility identifiers for existing settings, scripts and authenticated
//! connections. New UI, files and writes use Schnauz; keep these read fallbacks.
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub(super) const SFTP_KEYCHAIN_SERVICE: &str = "shuffle-studio-sftp";

pub(super) fn config_override() -> Option<OsString> {
    std::env::var_os("GETTER_CONFIG_DIR").or_else(|| std::env::var_os("SHUFFLE_STUDIO_CONFIG_DIR"))
}

pub(super) fn start_override() -> Option<OsString> {
    std::env::var_os("GETTER_START_DIR").or_else(|| std::env::var_os("SHUFFLE_STUDIO_START_DIR"))
}

pub(super) fn existing_config_dir(support: &Path) -> Option<PathBuf> {
    ["Getter", "Shuffle Studio"]
        .into_iter()
        .map(|name| support.join(name))
        .find(|path| path.is_dir())
}

pub(super) fn sftp_control_path(hash: u64) -> PathBuf {
    PathBuf::from(format!("/tmp/shuffle-studio-ssh-{hash:016x}.sock"))
}

pub(super) fn script_header_value(line: &str, key: &str) -> Option<String> {
    let prefix = format!("@shuffle.{key}:");
    line.to_lowercase()
        .starts_with(&prefix)
        .then(|| line[prefix.len()..].trim().to_string())
}

pub(super) fn script_environment(dir: &Path, paths: &str) -> [(&'static str, OsString); 2] {
    [
        ("SHUFFLE_DIR", dir.as_os_str().to_owned()),
        ("SHUFFLE_PATHS", paths.into()),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn existing_script_headers_keep_working_after_rename() {
        assert_eq!(
            crate::script_header_value("@schnauz.name: New action", "name").as_deref(),
            Some("New action")
        );
        assert_eq!(
            crate::script_header_value("@shuffle.NAME: Existing action", "name").as_deref(),
            Some("Existing action")
        );
        assert_eq!(
            crate::script_header_value("@other.name: Unrelated", "name"),
            None
        );
    }
}

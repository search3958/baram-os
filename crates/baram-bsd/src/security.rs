//! Permission checks for application-initiated OS setting changes.

use alloc::string::String;
#[cfg(feature = "security-hash")]
use alloc::string::ToString;

use crate::{app, uri};
#[cfg(feature = "security-hash")]
use crate::{config, vfs};

#[cfg(feature = "security-hash")]
const ALLOWED_HASHES_PATH: &str = "security/os-settings/allowed-app-blake3";

pub fn is_settings_write(uri_text: &str) -> bool {
    uri::parse(uri_text).is_some_and(|command| !command.params.is_empty())
}

/// Hash the exact packaged application bytes. For W3A this covers the whole
/// tar archive, including every UI, script, and configuration member.
pub fn app_hash(app_name: &str) -> Option<String> {
    if !app::is_safe_app_filename(app_name) {
        return None;
    }
    #[cfg(not(feature = "security-hash"))]
    {
        // Xiao has no permission dialog and does not retain application
        // digests. Returning an allowed marker keeps the shared URI path
        // non-blocking if it is ever used by the kiosk runtime.
        return Some(String::new());
    }
    #[cfg(feature = "security-hash")]
    {
    let bytes = vfs::read_file(&alloc::format!("apps/{app_name}"));
    (!bytes.is_empty()).then(|| blake3::hash(&bytes).to_hex().to_string())
    }
}

pub fn is_always_allowed(hash: &str) -> bool {
    #[cfg(not(feature = "security-hash"))]
    {
        let _ = hash;
        return true;
    }
    #[cfg(feature = "security-hash")]
    {
    config::get_config()
        .get(ALLOWED_HASHES_PATH)
        .unwrap_or("")
        .split(',')
        .any(|saved| !saved.is_empty() && saved == hash)
    }
}

pub fn allow_always(hash: &str) {
    #[cfg(not(feature = "security-hash"))]
    {
        let _ = hash;
        return;
    }
    #[cfg(feature = "security-hash")]
    {
    if !is_blake3_hex(hash) || is_always_allowed(hash) {
        return;
    }
    let current = config::get_config()
        .get(ALLOWED_HASHES_PATH)
        .unwrap_or("")
        .to_string();
    let updated = if current.is_empty() {
        hash.to_string()
    } else {
        alloc::format!("{current},{hash}")
    };
    config::update_and_save(|settings| settings.set(ALLOWED_HASHES_PATH, &updated));
    }
}

#[cfg(feature = "security-hash")]
fn is_blake3_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "security-hash")]
    #[test]
    fn blake3_matches_standard_vectors() {
        assert_eq!(
            blake3::hash(b"").to_hex().as_str(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        assert_eq!(
            blake3::hash(b"abc").to_hex().as_str(),
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        );
    }

    #[test]
    fn only_os_uris_with_parameters_are_writes() {
        assert!(is_settings_write("os://display/hud?enabled=1"));
        assert!(!is_settings_write("os://display/hud/enabled"));
        assert!(!is_settings_write("app://settings.w3a"));
    }
}

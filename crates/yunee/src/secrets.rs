//! Where the Canvas token lives.
//!
//! Primary home is the GNOME keyring via libsecret. If the Secret Service is
//! unreachable (no D-Bus, headless, keyring locked), Yunee falls back to a
//! `0600` file in its data directory so the app is still usable — and says so.

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use anyhow::{Context, Result};
use libsecret::{
    Schema, SchemaAttributeType, SchemaFlags, password_clear_sync, password_lookup_sync,
    password_store_sync,
};

use crate::paths;

const SCHEMA_NAME: &str = "io.github.LyKhoris.Yunee.CanvasToken";
const ATTR_SERVER: &str = "server";
const LABEL: &str = "Yunee — Canvas access token";

fn schema() -> Schema {
    let mut attributes = HashMap::new();
    attributes.insert(ATTR_SERVER, SchemaAttributeType::String);
    Schema::new(SCHEMA_NAME, SchemaFlags::NONE, attributes)
}

fn fallback_path() -> PathBuf {
    paths::data_dir().join("canvas-token")
}

fn write_fallback(token: &str) -> Result<()> {
    let path = fallback_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| format!("could not open {}", path.display()))?;
    file.write_all(token.as_bytes())?;
    Ok(())
}

fn read_fallback() -> Option<String> {
    std::fs::read_to_string(fallback_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn remove_fallback() {
    let _ = std::fs::remove_file(fallback_path());
}

/// Store the token for a server. Returns `true` if it landed in the keyring,
/// `false` if it fell back to a file.
pub fn store(server: &str, token: &str) -> Result<bool> {
    let schema = schema();
    let attributes = HashMap::from([(ATTR_SERVER, server)]);
    match password_store_sync(
        Some(&schema),
        attributes,
        None,
        LABEL,
        token,
        None::<&gtk4::gio::Cancellable>,
    ) {
        Ok(()) => {
            remove_fallback();
            Ok(true)
        }
        Err(_) => {
            write_fallback(token)?;
            Ok(false)
        }
    }
}

/// Look up the token for a server (keyring first, then file).
pub fn lookup(server: &str) -> Option<String> {
    let schema = schema();
    let attributes = HashMap::from([(ATTR_SERVER, server)]);
    if let Ok(Some(value)) =
        password_lookup_sync(Some(&schema), attributes, None::<&gtk4::gio::Cancellable>)
    {
        let value = value.to_string();
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    read_fallback()
}

/// Forget the token entirely.
pub fn clear(server: &str) {
    let schema = schema();
    let attributes = HashMap::from([(ATTR_SERVER, server)]);
    let _ = password_clear_sync(Some(&schema), attributes, None::<&gtk4::gio::Cancellable>);
    remove_fallback();
}

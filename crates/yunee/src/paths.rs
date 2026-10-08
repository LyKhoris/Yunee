//! Where Yunee keeps its files. Single-user, XDG, no hidden cloud.

use std::path::PathBuf;

/// `$XDG_DATA_HOME/yunee`, or `~/.local/share/yunee`.
///
/// `YUNEE_DATA_DIR` overrides everything — used to keep test runs out of the
/// real profile.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("YUNEE_DATA_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".local/share")
        }
    };
    base.join("yunee")
}

/// The SQLite database.
pub fn db_path() -> PathBuf {
    data_dir().join("yunee.db")
}

/// Where downloaded course files land.
pub fn downloads_dir() -> PathBuf {
    data_dir().join("files")
}

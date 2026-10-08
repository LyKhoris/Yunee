//! Shared state: the local store, and the (optional) Canvas connection.

use std::sync::Arc;

use anyhow::Result;
use yunee_canvas::CanvasClient;
use yunee_store::Store;

use crate::secrets;

const KEY_BASE_URL: &str = "canvas.base_url";

/// A saved, usable Canvas connection.
#[derive(Clone)]
pub struct Connection {
    pub base_url: String,
    pub token: String,
}

impl Connection {
    /// Build an API client for this connection.
    pub fn client(&self) -> Result<CanvasClient> {
        Ok(CanvasClient::new(&self.base_url, &self.token)?)
    }
}

/// Everything the app shares between the UI thread and background work.
pub struct AppState {
    pub store: Arc<Store>,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// The configured connection, if the base URL is set and a token exists.
    pub fn connection(&self) -> Option<Connection> {
        let base_url = self
            .store
            .get_setting(KEY_BASE_URL)
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())?;
        let token = secrets::lookup(&base_url)?;
        Some(Connection { base_url, token })
    }

    /// Persist a connection. Returns `true` when the token reached the keyring,
    /// `false` when it fell back to a protected file.
    pub fn save_connection(&self, base_url: &str, token: &str) -> Result<bool> {
        self.store.set_setting(KEY_BASE_URL, base_url)?;
        secrets::store(base_url, token)
    }

    pub fn clear_connection(&self) {
        if let Ok(Some(base)) = self.store.get_setting(KEY_BASE_URL) {
            secrets::clear(&base);
        }
        let _ = self.store.set_setting(KEY_BASE_URL, "");
    }
}

//! Shared state: the local store, and the (optional) Canvas connection.

use std::sync::{Arc, Mutex};

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
    client: Arc<Mutex<Option<Arc<CanvasClient>>>>,
}

impl Connection {
    pub fn new(base_url: String, token: String) -> Self {
        Self {
            base_url,
            token,
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Clones share the HTTP connection pool, request queue and cooldown.
    pub fn client(&self) -> Result<Arc<CanvasClient>> {
        let mut cached = self.client.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(client) = cached.as_ref() {
            return Ok(client.clone());
        }
        let client = Arc::new(CanvasClient::new(&self.base_url, &self.token)?);
        *cached = Some(client.clone());
        Ok(client)
    }

    /// A connection from `CANVAS_URL` + `CANVAS_TOKEN`, for development and
    /// scripted runs. Never persisted — the token stays in the environment.
    pub fn from_env() -> Option<Connection> {
        let base_url = std::env::var("CANVAS_URL").ok()?;
        let token = std::env::var("CANVAS_TOKEN").ok()?;
        if base_url.trim().is_empty() || token.trim().is_empty() {
            return None;
        }
        Some(Connection::new(base_url, token))
    }
}

/// Everything the app shares between the UI thread and background work.
pub struct AppState {
    pub store: Arc<Store>,
    connection_cache: Mutex<Option<Connection>>,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(store),
            connection_cache: Mutex::new(None),
        }
    }

    /// The configured connection, if the base URL is set and a token exists.
    /// `CANVAS_URL` + `CANVAS_TOKEN` in the environment take precedence, so a
    /// development run can borrow a token without saving it.
    pub fn connection(&self) -> Option<Connection> {
        if let Some(connection) = Connection::from_env() {
            return Some(self.reuse_connection(connection));
        }
        let base_url = self
            .store
            .get_setting(KEY_BASE_URL)
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())?;
        let token = secrets::lookup(&base_url)?;
        Some(self.reuse_connection(Connection::new(base_url, token)))
    }

    fn reuse_connection(&self, connection: Connection) -> Connection {
        let mut cached = self
            .connection_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(old) = cached.as_ref() {
            if old.base_url == connection.base_url && old.token == connection.token {
                return old.clone();
            }
        }
        *cached = Some(connection.clone());
        connection
    }

    /// The configured Canvas address, even when no usable token is available —
    /// enough to build "open on Canvas" links.
    pub fn base_url(&self) -> Option<String> {
        if let Some(connection) = Connection::from_env() {
            return Some(connection.base_url);
        }
        self.store
            .get_setting(KEY_BASE_URL)
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
    }

    /// Persist a connection. Returns `true` when the token reached the keyring,
    /// `false` when it fell back to a protected file.
    pub fn save_connection(&self, base_url: &str, token: &str) -> Result<bool> {
        self.store.set_setting(KEY_BASE_URL, base_url)?;
        secrets::store(base_url, token)
    }

    pub fn clear_connection(&self) {
        self.connection_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Ok(Some(base)) = self.store.get_setting(KEY_BASE_URL) {
            secrets::clear(&base);
        }
        let _ = self.store.set_setting(KEY_BASE_URL, "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_clones_and_repeated_lookups_share_the_client() {
        let state = AppState::new(Store::open_in_memory().unwrap());
        let first = state.reuse_connection(Connection::new(
            "https://canvas.test".into(),
            "token".into(),
        ));
        let second = state.reuse_connection(Connection::new(
            "https://canvas.test".into(),
            "token".into(),
        ));
        assert!(Arc::ptr_eq(
            &first.client().unwrap(),
            &second.client().unwrap()
        ));
        assert!(Arc::ptr_eq(
            &first.client().unwrap(),
            &first.clone().client().unwrap()
        ));
        let rotated = state.reuse_connection(Connection::new(
            "https://canvas.test".into(),
            "new-token".into(),
        ));
        assert!(!Arc::ptr_eq(
            &first.client().unwrap(),
            &rotated.client().unwrap()
        ));
    }
}

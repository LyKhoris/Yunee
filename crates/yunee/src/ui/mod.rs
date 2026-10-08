//! The GTK4 + libadwaita user interface.

pub mod app;
pub(crate) mod connect;
pub(crate) mod widgets;

pub use app::{APP_ID, build};

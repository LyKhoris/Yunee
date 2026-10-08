//! The GTK4 + libadwaita user interface.

pub mod app;
pub(crate) mod connect;
pub(crate) mod course;
pub(crate) mod dashboard;
pub(crate) mod detail;
pub(crate) mod settings;
pub(crate) mod widgets;

pub use app::{APP_ID, build};

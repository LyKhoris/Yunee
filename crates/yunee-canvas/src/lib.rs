//! `yunee-canvas` — a typed client for the Canvas LMS REST API.
//!
//! Scope is a single student's own account: everything they can read, plus the
//! writes Canvas documents for students (submissions, module completion,
//! planner overrides, read markers). It fixes the three things that made the
//! earlier client fragile:
//!
//! * **Pagination** — every list call follows `Link: rel="next"` to the end.
//! * **Throttling** — `X-Rate-Limit-Remaining` is watched and 403/429 back off.
//! * **Feature detection** — a 404 surfaces as [`CanvasError::NotFound`] so
//!   callers can degrade on older/self-hosted instances instead of crashing.
//!
//! The crate has no storage or UI dependency; it only speaks HTTP and JSON.

pub mod client;
pub mod error;
pub mod ids;
pub mod pagination;
pub mod types;

pub use client::{CanvasClient, normalize_base};
pub use error::CanvasError;
pub use types::*;

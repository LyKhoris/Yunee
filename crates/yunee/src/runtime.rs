//! One shared Tokio runtime for every network operation.
//!
//! The GTK main loop is not async. Background work (sync, uploads, downloads)
//! runs on this runtime from a plain thread, then hands results back to the UI
//! thread through the component's sender.

use std::sync::OnceLock;
use tokio::runtime::Runtime;

/// The process-wide runtime, created on first use.
pub fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("yunee-net")
            .build()
            .expect("failed to start the async runtime")
    })
}

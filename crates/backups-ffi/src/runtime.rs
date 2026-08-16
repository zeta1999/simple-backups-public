//! Shared Tokio runtime for blocking FFI entry points.

use std::sync::OnceLock;
use tokio::runtime::Runtime;

pub fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("simple-backups-ffi")
            .build()
            .expect("tokio runtime")
    })
}

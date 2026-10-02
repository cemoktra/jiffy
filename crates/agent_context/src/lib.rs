//! Local context used by the third-party CLI agent integration: skill discovery for the rich
//! input skill menu, codebase symbol outlines for `@`-mentions, and workspace metadata.
pub mod outline;
pub mod skills;
pub mod workspace;

#[cfg(feature = "local_fs")]
use native::*;
#[cfg(feature = "local_fs")]
pub use outline::build_outline;
pub use outline::{Outline, Symbol};
#[cfg(not(feature = "local_fs"))]
use wasm::*;

#[cfg(feature = "local_fs")]
mod native {
    use std::thread::available_parallelism;

    pub(super) const MAX_PARALLEL_THREADS: usize = 2;

    fn create_thread_pool() -> Option<rayon::ThreadPool> {
        let num_threads = available_parallelism()
            .map(|parallelism| (parallelism.get() / 2).clamp(1, MAX_PARALLEL_THREADS))
            .unwrap_or(MAX_PARALLEL_THREADS);

        rayon::ThreadPoolBuilder::new()
            .thread_name(|index| format!("warp-code-outline-{index}"))
            .num_threads(num_threads)
            .build()
            .ok()
    }

    lazy_static::lazy_static! {
        pub(super) static ref THREADPOOL: Option<rayon::ThreadPool> = create_thread_pool();
    }
}

#[cfg(not(feature = "local_fs"))]
mod wasm {
    lazy_static::lazy_static! {
        pub(super) static ref THREADPOOL: Option<rayon::ThreadPool> = None;
    }
}

//! A wait-free [`AtomicArc`] optimized for read-intensive use cases.
//!
//! # Examples
//!
//! ```ignore
//! use hazarc::AtomicArc;
//!
//! struct Config {/* ... */}
//!
//! fn update_config(shared_cfg: &AtomicArc<Config> /* ... */) {
//!     shared_cfg.store(/* ... */);
//! }
//!
//! fn task(shared_cfg: &AtomicArc<Config>) {
//!     loop {
//!         let cfg = shared_cfg.load();
//!         /* ... */
//!     }
//! }
//! ```
//!
//! `AtomicArc::load` is already very fast, but `Cache::load` is blazingly fast.
//!
//! ```ignore
//! use std::sync::Arc;
//!
//! use hazarc::AtomicArc;
//!
//! struct Config {/* ... */}
//!
//! fn update_config(shared_cfg: &AtomicArc<Config> /* ... */) {
//!     shared_cfg.store(/* ... */);
//! }
//!
//! fn spawn_task(shared_cfg: Arc<AtomicArc<Config>>) {
//!     thread::spawn(move || {
//!         let mut cache = hazarc::Cache::new(shared_cfg);
//!         loop {
//!             let cfg = cache.load();
//!             /* ... */
//!         }
//!     });
//! }
//! ```
//!
//! With custom domains, it can be used in a `no_std` environment.
//!
//! ```ignore
//! #![no_std]
//! extern crate alloc;
//!
//! use alloc::sync::Arc;
//!
//! use hazarc::{domain::Domain, AtomicArc};
//!
//! hazarc::pthread_domain!(NoStdDomain(2)); // 2 hazard pointer slots
//! fn register_domain_cleanup() {
//!     extern "C" fn deallocate_domain() {
//!         unsafe { NoStdDomain::static_list().deallocate() };
//!     }
//!     unsafe { libc::atexit(deallocate_domain) };
//! }
//!
//! struct Config {/* ... */}
//!
//! fn update_config(shared_cfg: &AtomicArc<Config, NoStdDomain> /* ... */) {
//!     shared_cfg.store(/* ... */);
//! }
//!
//! fn task(shared_cfg: &AtomicArc<Config, NoStdDomain>) {
//!     loop {
//!         let cfg = shared_cfg.load();
//!         /* ... */
//!     }
//! }
//! ```
//!
//! # Write policy
//!
//! `AtomicArc` has a generic `WritePolicy` parameter with the following variants:
//! - `Serialized`: writes on a given `AtomicArc` should be serialized — with a mutex, a MPSC task,
//!   etc. Concurrent writes are still safe with this policy, but can provoke non-monotonic reads,
//!   i.e. a subsequent load may observe an older value than a previous load.
//! - `Concurrent` (the default): writes on a given `AtomicArc` can be concurrent. This adds a
//!   small overhead to the non-critical path of reads, and a larger overhead to writes on 32-bit
//!   platforms.
//! # Wait-freedom
//!
//! ### Load
//!
//! #### Thread-local access
//!
//! [`AtomicArc::load`] relies on a domain's thread-local node which is lazily allocated and
//! inserted in the domain's global list on first access. As a consequence, the first
//! `AtomicArc::load` for a given domain may not be wait-free.
//!
//! It is however possible to [access the thread-local node] explicitly before using `AtomicArc`,
//! making all subsequent accesses wait-free. Another solution is to pre-allocate the number of
//! nodes required by the program. In that case, insertion into the domain's global list is bounded
//! by the number of allocated nodes, and thread-local accesses are wait-free.
//!
//! #### Concurrent writes on 32-bit platforms
//!
//! Concurrent writes require handling the ABA problem with a generation counter, which can
//! overflow on 32-bit platforms. On overflow, the thread-local node has to be released, and
//! subsequent `AtomicArc::load` call may allocate a new node. As a consequence, wait-freedom is
//! only guaranteed for at least 2^31 consecutive loads.
//!
//! ### Store
//!
//! [`AtomicArc::store`], which wraps [`AtomicArc::swap`], scans the whole domain's global list,
//! executing a bounded number of atomic operations on each node. If the number of nodes is bounded
//! as well — which should be the case most of the time — then the whole operation is wait-free.
//!
//! If there are concurrent writes on the same `AtomicArc`, they may execute `AtomicArc::load`
//! internally, with the same consequences on wait-freedom.
//!
//! [access the thread-local node]: domain::Domain::get_or_acquire_thread_local_node
//!
//! # Safety
//!
//! This library uses unsafe code to deal with `AtomicPtr` manipulation and DST allocations.
//! It is extensively tested with [`miri`](https://github.com/rust-lang/miri) to ensure its
//! soundness, including over multiple weak memory model permutations.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![no_std]
extern crate alloc;
#[cfg(any(feature = "default-domain", test))]
extern crate std;

use alloc::sync::Arc;

pub mod arc;
pub mod atomic;
pub mod cache;
pub mod domain;
mod msrv;
#[cfg(feature = "serde")]
mod serde;
pub mod write_policy;

#[cfg(feature = "default-domain")]
domain! {
    #[cfg(feature = "default-domain")]
    /// Default domain with 8 borrow slots.
    pub DefaultDomain(8)
}
#[cfg(feature = "pthread-domain")]
#[doc(hidden)]
pub use libc;

/// Alias for `AtomicArcPtr<Arc<T>>`
#[cfg(feature = "default-domain")]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub type AtomicArc<T, D = DefaultDomain, W = write_policy::Concurrent> =
    atomic::AtomicArcPtr<Arc<T>, D, W>;
/// Alias for `AtomicArcPtr<Arc<T>>`
#[cfg(not(feature = "default-domain"))]
pub type AtomicArc<T, D, W = write_policy::Concurrent> = atomic::AtomicArcPtr<Arc<T>, D, W>;
/// Alias for `AtomicOptionArcPtr<Arc<T>>`
#[cfg(feature = "default-domain")]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub type AtomicOptionArc<T, D = DefaultDomain, W = write_policy::Concurrent> =
    atomic::AtomicOptionArcPtr<Arc<T>, D, W>;
/// Alias for `AtomicOptionArcPtr<Arc<T>>`
#[cfg(not(feature = "default-domain"))]
pub type AtomicOptionArc<T, D, W = write_policy::Concurrent> =
    atomic::AtomicOptionArcPtr<Arc<T>, D, W>;
/// Alias for `ArcPtrBorrow<Arc<T>>`
pub type ArcBorrow<T> = atomic::ArcPtrBorrow<Arc<T>>;

pub use cache::Cache;

const NULL: *mut () = core::ptr::null_mut();

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
//! hazarc::pthread_domain!(NoStdDomain(slots = 2));
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
//! # Features
//!
//! - `default-domain` *(enabled by default)*: provides a default domain parameter to [`AtomicArc`]
//!   using standard thread-local storage.
//! - `domain-gc`: enables automatic domain deallocation.
//! - `membarrier`: enables `domain::Membarrier` synchronization, using process-wide memory barriers
//!   (Linux, Android, FreeBSD, Windows and Apple platforms only; on ARM, only AArch64 with `rcpc`).
//! - `portable-atomic`: uses [`portable-atomic`](https://docs.rs/portable-atomic) atomics instead
//!   of `core` ones.
//! - `portable-atomic-util`: enables [`portable_atomic_util::Arc`](https://docs.rs/portable-atomic-util/latest/portable_atomic_util/struct.Arc.html) support.
//! - `pthread-domain`: enables the `pthread_domain!` macro for `no_std` environments using
//!   POSIX pthread thread-local storage (Unix only).
//! - `serde`: enables `serde` support.
//! - `triomphe`: enables [`triomphe::Arc`](https://docs.rs/triomphe/latest/triomphe/struct.Arc.html) support.
//!
//! # Write policy
//!
//! `AtomicArc` has a generic [`WritePolicy`] parameter with the following variants:
//! - [`Serialized`]: writes on a given `AtomicArc` should be serialized — with a mutex, a MPSC
//!   task, etc. Concurrent writes are still safe with this policy, but can provoke non-monotonic
//!   reads, i.e. a subsequent load may observe an older value than a previous load.
//! - [`Concurrent`] (the default): writes on a given `AtomicArc` can be concurrent. This adds a
//!   small overhead to the non-critical path of reads, and a larger overhead to writes on 32-bit
//!   platforms.
//!
//! [`WritePolicy`]: write_policy::WritePolicy
//! [`Concurrent`]: write_policy::Concurrent
//! [`Serialized`]: write_policy::Serialized
//!
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
//! ### `Arc` deallocation
//!
//! In case of a concurrent write, a loaded [`ArcBorrow`] might contain the last reference to the
//! underlying `Arc`, which is thus deallocated on drop. In some edge cases, an `Arc` can also be
//! de allocated during `AtomicArc::load`.
//!
//! Those deallocations are not part of the algorithm itself, which only drop the [`ArcPtr`]
//! instances.
//!
//! #### Concurrent writes on 32-bit platforms
//!
//! `Concurrent` writes require handling the ABA problem with a generation counter, which can
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
//! [access the thread-local node]: domain::DomainExt::get_or_acquire_thread_local_node
//! [`ArcPtr`]: arc::ArcPtr
//!
//! # Memory ordering
//!
//! The ordering of `AtomicArc` operations depends on the domain's [`Synchronization`]:
//! - [`Sequential`]: all operations are [`SeqCst`].
//! - [`AsymmetricBarriers`] implementations, e.g. `Membarrier` with `membarrier` feature: loads
//!   are [`Acquire`], preceded by a [light barrier], and swaps are [`AcqRel`], followed by a
//!   [heavy barrier]. The barriers make it possible to use `AtomicArc` in a Dekker algorithm.
//!   However, loads and swaps are not part of the `SeqCst` total order, so independent reads of
//!   independent writes (IRIW) may observe the writes in different orders.
//!
//! [`Synchronization`]: domain::Domain::Synchronization
//! [`Sequential`]: domain::Sequential
//! [`AsymmetricBarriers`]: domain::AsymmetricBarriers
//! [light barrier]: domain::AsymmetricBarriers::light_barrier
//! [heavy barrier]: domain::AsymmetricBarriers::heavy_barrier
//! [`SeqCst`]: core::sync::atomic::Ordering::SeqCst
//! [`Acquire`]: core::sync::atomic::Ordering::Acquire
//! [`AcqRel`]: core::sync::atomic::Ordering::AcqRel
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![no_std]
extern crate alloc;
#[cfg(any(feature = "default-domain", feature = "membarrier", test))]
extern crate std;

#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;

pub mod arc;
pub mod atomic;
pub mod cache;
pub mod domain;
mod loom;
#[cfg(feature = "membarrier")]
mod membarrier;
#[cfg(all(hazarc_assume_membarrier, not(feature = "membarrier")))]
compile_error!("`hazarc_assume_membarrier` cfg requires `membarrier` feature");
mod msrv;
#[cfg(feature = "serde")]
mod serde;
pub mod write_policy;

#[cfg(feature = "default-domain")]
domain! {
    #[cfg(feature = "default-domain")]
    /// Default domain with [`DEFAULT_BORROW_SLOT_COUNT`](domain::DEFAULT_BORROW_SLOT_COUNT)
    /// borrow slots.
    pub DefaultDomain
}
#[cfg(loom)]
#[doc(hidden)]
pub use ::loom as __loom;
#[cfg(all(feature = "pthread-domain", unix))]
#[doc(hidden)]
pub use libc;

/// Alias for `AtomicArcPtr<Arc<T>>`
#[cfg(all(feature = "default-domain", target_has_atomic = "ptr"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub type AtomicArc<T, D = DefaultDomain, W = write_policy::Concurrent> =
    atomic::AtomicArcPtr<Arc<T>, D, W>;
/// Alias for `AtomicArcPtr<Arc<T>>`
#[cfg(all(not(feature = "default-domain"), target_has_atomic = "ptr"))]
pub type AtomicArc<T, D, W = write_policy::Concurrent> = atomic::AtomicArcPtr<Arc<T>, D, W>;
/// Alias for `ArcPtrBorrow<Arc<T>>`
#[cfg(all(feature = "default-domain", target_has_atomic = "ptr"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub type ArcBorrow<T, D = DefaultDomain> = atomic::ArcPtrBorrow<Arc<T>, D>;
/// Alias for `ArcPtrBorrow<Arc<T>>`
#[cfg(all(not(feature = "default-domain"), target_has_atomic = "ptr"))]
pub type ArcBorrow<T, D> = atomic::ArcPtrBorrow<Arc<T>, D>;

pub use cache::Cache;

const NULL: *mut () = core::ptr::null_mut();

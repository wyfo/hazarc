//! A wait-free [`AtomicArc`] optimized for read-intensive use cases.
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
#![cfg_attr(docsrs, feature(doc_cfg))]
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

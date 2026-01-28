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
pub mod load_policy;

#[cfg(feature = "default-domain")]
domain!(pub DefaultDomain(8));
#[cfg(feature = "pthread-domain")]
#[doc(hidden)]
pub use libc;

#[cfg(feature = "default-domain")]
pub type AtomicArc<T, D = DefaultDomain, P = load_policy::Adaptive> =
    atomic::AtomicArcPtr<Arc<T>, D, P>;
#[cfg(not(feature = "default-domain"))]
pub type AtomicArc<T, D, P = load_policy::Adaptive> = atomic::AtomicArcPtr<Arc<T>, D, P>;
#[cfg(feature = "default-domain")]
pub type AtomicOptionArc<T, D = DefaultDomain, P = load_policy::Adaptive> =
    atomic::AtomicOptionArcPtr<Arc<T>, D, P>;
#[cfg(not(feature = "default-domain"))]
pub type AtomicOptionArc<T, D, P = load_policy::Adaptive> =
    atomic::AtomicOptionArcPtr<Arc<T>, D, P>;
pub type ArcBorrow<T> = atomic::ArcPtrBorrow<Arc<T>>;

pub use cache::Cache;

const NULL: *mut () = core::ptr::null_mut();

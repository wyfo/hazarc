#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]
extern crate alloc;

use alloc::sync::Arc;

pub mod arc;
pub mod atomic;
pub mod borrow_list;
pub mod cache;

#[cfg(feature = "default-borrow-list")]
borrow_list!(pub DefaultBorrowList(8));

#[cfg(feature = "default-borrow-list")]
pub type AtomicArc<T, L = DefaultBorrowList> = atomic::AtomicArcPtr<Arc<T>, L>;
#[cfg(not(feature = "default-borrow-list"))]
pub type AtomicArc<T, L> = atomic::AtomicArcPtr<Arc<T>, L>;
#[cfg(feature = "default-borrow-list")]
pub type AtomicOptionArc<T, L = DefaultBorrowList> = atomic::AtomicArcPtr<Option<Arc<T>>, L>;
#[cfg(not(feature = "default-borrow-list"))]
pub type AtomicOptionArc<T, L> = atomic::AtomicArcPtr<Option<Arc<T>>, L>;
pub type ArcBorrow<T> = atomic::ArcPtrBorrow<Arc<T>>;

const NULL: *mut () = core::ptr::null_mut();

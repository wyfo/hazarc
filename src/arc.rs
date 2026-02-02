//! Generic traits for `Arc`-like pointer types

use alloc::sync::{Arc, Weak};
use core::{mem::ManuallyDrop, ops::Deref, pin::Pin, ptr};

#[allow(unused_imports)]
use crate::msrv::StrictProvenance;
use crate::{atomic::ArcPtrBorrow, NULL};

/// An `Arc`-like pointer.
///
/// # Safety
///
/// Implementors must have the same semantics as [`Arc`]. The pointer
/// returned by [`into_ptr`](Self::into_ptr) **must be aligned to 4**.
pub unsafe trait ArcPtr: Clone {
    /// Whether this pointer type may be null.
    const NULLABLE: bool = false;
    /// Constructs an Arc pointer from a raw pointer.
    ///
    /// # Safety
    ///
    /// The pointer must have been obtained from [`into_ptr`](Self::into_ptr)
    /// or [`as_ptr`](Self::as_ptr), and the reference count must be greater
    /// than or equal to 1.
    unsafe fn from_ptr(ptr: *mut ()) -> Self;
    /// Consumes the Arc pointer, returning the wrapped pointer.
    #[allow(clippy::wrong_self_convention)]
    fn into_ptr(arc: Self) -> *mut ();
    /// Returns the wrapped pointer without consuming the Arc pointer.
    #[allow(clippy::wrong_self_convention)]
    fn as_ptr(arc: &Self) -> *mut ();
    /// Increments the reference count of the Arc pointer.
    ///
    /// # Safety
    ///
    /// Same precondition as [`from_ptr`](Self::from_ptr).
    #[inline(always)]
    unsafe fn incr_rc(ptr: *mut ()) {
        let _ = unsafe { ManuallyDrop::new(Self::from_ptr(ptr)).clone() };
    }
    /// Decrements the reference count of the Arc pointer.
    ///
    /// # Safety
    ///
    /// Same precondition as [`from_ptr`](Self::from_ptr).
    #[inline(always)]
    unsafe fn decr_rc(ptr: *mut ()) {
        unsafe { drop(Self::from_ptr(ptr)) }
    }
}

unsafe impl<A: NonNullArcPtr> ArcPtr for Option<A> {
    const NULLABLE: bool = true;
    #[inline(always)]
    unsafe fn from_ptr(ptr: *mut ()) -> Self {
        // SAFETY: same function contract
        (!ptr.is_null()).then(|| unsafe { A::from_ptr(ptr) })
    }
    #[inline(always)]
    fn into_ptr(arc: Self) -> *mut () {
        arc.map_or(NULL, A::into_ptr)
    }
    #[inline(always)]
    fn as_ptr(arc: &Self) -> *mut () {
        arc.as_ref().map_or(NULL, A::as_ptr)
    }
}

unsafe impl<A: ArcPtr + Deref> ArcPtr for Pin<A> {
    #[inline(always)]
    unsafe fn from_ptr(ptr: *mut ()) -> Self {
        unsafe { Pin::new_unchecked(A::from_ptr(ptr)) }
    }
    #[inline(always)]
    fn into_ptr(arc: Self) -> *mut () {
        unsafe { A::into_ptr(Pin::into_inner_unchecked(arc)) }
    }
    #[inline(always)]
    fn as_ptr(arc: &Self) -> *mut () {
        unsafe {
            let arc = ManuallyDrop::new(Pin::into_inner_unchecked(ptr::read(arc)));
            A::as_ptr(&arc)
        }
    }
}

#[cfg(not(target_pointer_width = "16"))]
unsafe impl<T> ArcPtr for Arc<T> {
    #[inline(always)]
    unsafe fn from_ptr(ptr: *mut ()) -> Self {
        unsafe { Arc::from_raw(ptr.cast()) }
    }
    #[inline(always)]
    fn into_ptr(arc: Self) -> *mut () {
        Arc::into_raw(arc).cast_mut().cast()
    }
    #[inline(always)]
    fn as_ptr(arc: &Self) -> *mut () {
        Arc::as_ptr(arc).cast_mut().cast()
    }
}

#[cfg(not(target_pointer_width = "16"))]
unsafe impl<T> ArcPtr for Weak<T> {
    #[inline(always)]
    unsafe fn from_ptr(ptr: *mut ()) -> Self {
        unsafe { Weak::from_raw(ptr.cast()) }
    }
    #[inline(always)]
    fn into_ptr(arc: Self) -> *mut () {
        Weak::into_raw(arc).cast_mut().cast()
    }
    #[inline(always)]
    fn as_ptr(arc: &Self) -> *mut () {
        Weak::as_ptr(arc).cast_mut().cast()
    }
}

/// An [`ArcPtr`] whose [`ArcPtr::into_ptr`] never returns a null pointer.
///
/// # Safety
///
/// [`ArcPtr::into_ptr`] must return a non-null pointer; [`ArcPtr::NULLABLE`] must be `false`.
pub unsafe trait NonNullArcPtr: ArcPtr {}

unsafe impl<A: NonNullArcPtr + Deref> NonNullArcPtr for Pin<A> {}

unsafe impl<T> NonNullArcPtr for Arc<T> {}

/// A reference to an [`ArcPtr`].
///
/// This trait exists to allow `Option<&A>` to be used when `&Option<A>` would be expected,
/// e.g. [`AtomicArc::compare_exchange`](crate::AtomicArc::compare_exchange).
pub trait ArcRef<A: ArcPtr> {
    /// Returns a pointer of the referenced [`ArcPtr`].
    #[allow(clippy::wrong_self_convention)]
    fn as_ptr(this: Self) -> *mut ();
}

impl<A: ArcPtr> ArcRef<A> for &A {
    fn as_ptr(this: Self) -> *mut () {
        <A as ArcPtr>::as_ptr(this)
    }
}

impl<A: NonNullArcPtr> ArcRef<Option<A>> for &A {
    fn as_ptr(this: Self) -> *mut () {
        <A as ArcPtr>::as_ptr(this)
    }
}

impl<A: NonNullArcPtr> ArcRef<Option<A>> for Option<&A> {
    fn as_ptr(this: Self) -> *mut () {
        this.map_or(NULL, A::as_ptr)
    }
}

impl<A: ArcPtr> ArcRef<A> for &ArcPtrBorrow<A> {
    fn as_ptr(this: Self) -> *mut () {
        ArcRef::as_ptr(&**this)
    }
}

impl<A: NonNullArcPtr> ArcRef<Option<A>> for Option<&ArcPtrBorrow<A>> {
    fn as_ptr(this: Self) -> *mut () {
        ArcRef::as_ptr(this.map(AsRef::as_ref))
    }
}

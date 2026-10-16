//! Generic traits for `Arc`-like pointer types

use alloc::sync::{Arc, Weak};
use core::{mem, mem::ManuallyDrop, ops::Deref, pin::Pin, ptr};

#[allow(unused_imports)]
use crate::msrv::{ResultExt, StrictProvenance};
use crate::{atomic::ArcPtrBorrow, NULL};

/// An `Arc`-like pointer.
///
/// # Safety
///
/// Implementors must have the same semantics as [`Arc`]. The pointer returned by
/// [`into_ptr`](Self::into_ptr)/[`as_ptr`](Self::as_ptr) must be aligned to 4 and not be null
/// unless [`NULLABLE`](Self::NULLABLE) is set to true.
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
}

/// Type-erased drop function of an [`ArcPtr`] type.
///
/// There is an interleaving where a pointer loaded by an `AtomicArc` can be upgraded by another
/// `AtomicArc`, while the `Arc` has been reallocated in between, with possibly a different type,
/// or mutably borrowed.
///
/// As a consequence, the loaded pointer cannot be used for a borrow, and must be dropped. As its
/// exact type cannot be known, its destructor must be passed with the upgrade.
///
/// Wrapping the drop function pointer in a vtable ensures the required alignment.
#[repr(align(4))]
pub(crate) struct DropVTable {
    pub(crate) drop: unsafe fn(*mut ()),
}

pub(crate) trait ArcPtrExt: ArcPtr {
    const DROP_VTABLE: &'static DropVTable = &DropVTable {
        drop: |ptr| drop(unsafe { Self::from_ptr(ptr) }),
    };

    /// Clones the Arc and tries transferring the new reference ownership with the given operation.
    ///
    /// The clone's pointer is passed to the operation, and the clone is dropped if it returns
    /// `Err`.
    fn transfer_ownership<T, E>(&self, f: impl FnOnce(*mut ()) -> Result<T, E>) -> Result<T, E> {
        let clone = self.clone();
        f(Self::as_ptr(self)).inspect(|_| mem::forget(clone))
    }
}

impl<A: ArcPtr> ArcPtrExt for A {}

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

#[cfg(loom)]
unsafe impl<T> ArcPtr for loom::sync::Arc<T> {
    #[inline(always)]
    unsafe fn from_ptr(ptr: *mut ()) -> Self {
        unsafe { loom::sync::Arc::from_raw(ptr.cast()) }
    }
    #[inline(always)]
    fn into_ptr(arc: Self) -> *mut () {
        loom::sync::Arc::into_raw(arc).cast_mut().cast()
    }
    #[inline(always)]
    fn as_ptr(arc: &Self) -> *mut () {
        loom::sync::Arc::as_ptr(arc).cast_mut().cast()
    }
}

#[cfg(loom)]
unsafe impl<T> NonNullArcPtr for loom::sync::Arc<T> {}

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

/// An [`ArcPtr`] whose underlying pointer is never null.
///
/// # Safety
///
/// [`ArcPtr::NULLABLE`] must be `false`.
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

use alloc::sync::Arc;
use core::{mem::ManuallyDrop, ops::Deref, pin::Pin, ptr};

use crate::NULL;

#[allow(clippy::missing_safety_doc)]
pub unsafe trait NonNullPtr {}

unsafe impl<A: ArcPtr + NonNullPtr> NonNullPtr for Pin<A> {}

unsafe impl<T> NonNullPtr for Arc<T> {}

#[allow(clippy::missing_safety_doc)]
pub unsafe trait ArcPtr: Clone {
    const NULLABLE: bool = false;
    unsafe fn from_ptr(ptr: *mut ()) -> Self;
    #[allow(clippy::wrong_self_convention)]
    fn into_ptr(arc: Self) -> *mut ();
    #[allow(clippy::wrong_self_convention)]
    fn as_ptr(arc: &Self) -> *mut ();
    #[inline(always)]
    unsafe fn incr_rc(ptr: *mut ()) {
        let _ = unsafe { ManuallyDrop::new(Self::from_ptr(ptr)).clone() };
    }
    #[inline(always)]
    unsafe fn decr_rc(ptr: *mut ()) {
        unsafe { drop(Self::from_ptr(ptr)) }
    }
}

unsafe impl<A: ArcPtr + NonNullPtr> ArcPtr for Option<A> {
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

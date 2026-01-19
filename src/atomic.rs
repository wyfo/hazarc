use alloc::sync::Arc;
use core::{
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::Deref,
    ptr,
    sync::atomic::{
        AtomicPtr,
        Ordering::{Acquire, Relaxed, Release, SeqCst},
    },
};

use crate::{
    NULL,
    arc::{ArcPtr, NonNullPtr},
    borrow_list::{Borrow, BorrowNodeRef, StaticBorrowList},
};

const TRY_LOAD_FLAG: usize = 0b01;
const CONFIRM_FLAG: usize = 0b10;

pub struct AtomicArcPtr<A: ArcPtr, L: StaticBorrowList> {
    ptr: AtomicPtr<()>,
    _arc: PhantomData<A>,
    _list: PhantomData<L>,
}

impl<A: ArcPtr, L: StaticBorrowList> AtomicArcPtr<A, L> {
    #[inline]
    pub fn new(arc: A) -> Self {
        Self {
            ptr: AtomicPtr::new(A::into_ptr(arc)),
            _arc: PhantomData,
            _list: PhantomData,
        }
    }

    #[inline(always)]
    fn load_impl(&self) -> ArcPtrBorrow<A> {
        self.load_with_ptr(self.ptr.load(Relaxed))
    }

    #[inline(always)]
    fn load_with_ptr(&self, ptr: *mut ()) -> ArcPtrBorrow<A> {
        if A::NULLABLE && ptr.is_null() {
            return ArcPtrBorrow::new(ptr, None);
        }
        debug_assert!(!ptr.is_null());
        let node = L::thread_local_node();
        let borrow_idx = node.next_borrow_idx.get();
        // `slots().get_unchecked` seems to ruin performance...
        let slot = unsafe { &*node.borrow_ptr().add(borrow_idx) };
        // let slot = unsafe { node.slots().get_unchecked(slot_idx) };
        if slot.load(Relaxed).is_null() {
            self.load_with_borrow(ptr, node, slot, borrow_idx)
        } else {
            self.load_find_available_borrow(ptr, node)
        }
    }

    #[inline(always)]
    fn load_with_borrow(
        &self,
        ptr: *mut (),
        node: BorrowNodeRef,
        borrow: &'static AtomicPtr<()>,
        borrow_idx: usize,
    ) -> ArcPtrBorrow<A> {
        node.next_borrow_idx
            .set((borrow_idx + 1) & node.borrow_idx_mask);
        borrow.store(ptr, SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        if ptr != ptr_checked {
            return self.load_outdated(node, ptr, borrow);
        }
        ArcPtrBorrow::new(ptr_checked, Some(borrow))
    }

    #[cold]
    #[inline(never)]
    fn load_outdated(
        &self,
        node: BorrowNodeRef,
        ptr: *mut (),
        borrow: &'static AtomicPtr<()>,
    ) -> ArcPtrBorrow<A> {
        match borrow.compare_exchange(ptr, NULL, Relaxed, Relaxed) {
            Ok(_) => self.load_fallback(node),
            Err(_) => ArcPtrBorrow::new(ptr, None),
        }
    }

    #[cold]
    #[inline(never)]
    fn load_find_available_borrow(&self, ptr: *mut (), node: BorrowNodeRef) -> ArcPtrBorrow<A> {
        match (node.borrows().iter().enumerate()).find(|(_, borrow)| borrow.load(Relaxed).is_null())
        {
            Some((slot_idx, slot)) => self.load_with_borrow(ptr, node, slot, slot_idx),
            None => self.load_fallback(node),
        }
    }

    fn load_fallback(&self, node: BorrowNodeRef) -> ArcPtrBorrow<A> {
        let try_load_ptr = (&self.ptr as *const _ as *mut ()).map_addr(|addr| addr | TRY_LOAD_FLAG);
        node.fallback.store(try_load_ptr, SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        let confirm_ptr = ptr_checked.map_addr(|addr| addr | CONFIRM_FLAG).cast();
        let mut ptr_confirmed = ptr_checked;
        match (node.fallback).compare_exchange(try_load_ptr, confirm_ptr, Relaxed, Acquire) {
            Ok(_) => match (node.fallback).compare_exchange(confirm_ptr, NULL, Relaxed, Acquire) {
                Ok(_) => unsafe { A::incr_rc(ptr_checked) },
                Err(ptr) => debug_assert!(ptr.is_null()),
            },
            Err(ptr) => ptr_confirmed = ptr.cast(),
        }
        ArcPtrBorrow::new(ptr_confirmed, None)
    }

    #[inline]
    pub fn load_owned(&self) -> A {
        self.load_impl().into_owned()
    }

    #[inline]
    pub fn load_if_outdated_impl<'a>(&self, arc: &'a A) -> Result<&'a A, ArcPtrBorrow<A>> {
        let ptr = self.ptr.load(Relaxed);
        if ptr == A::as_ptr(arc) {
            Ok(arc)
        } else {
            Err(self.load_with_ptr(ptr))
        }
    }

    pub fn swap(&self, arc: A) -> A {
        let new_ptr = A::into_ptr(arc);
        let old_ptr = self.ptr.swap(new_ptr, SeqCst);
        self.swap_impl(new_ptr, old_ptr)
    }

    fn swap_impl(&self, new_ptr: *mut (), old_ptr: *mut ()) -> A {
        let old_arc = unsafe { A::from_ptr(old_ptr) };
        // increment the refcount before in case some slots are reset
        let _guard = old_arc.clone();
        for node in L::static_list().nodes() {
            if !A::NULLABLE || !old_ptr.is_null() {
                for borrow in node.borrows().iter() {
                    if borrow.load(SeqCst) == old_ptr.cast()
                        && (borrow.compare_exchange(old_ptr.cast(), NULL, Release, Relaxed)).is_ok()
                    {
                        unsafe { A::incr_rc(old_ptr) }
                    }
                }
            }
            let fallback_ptr = node.fallback.load(SeqCst);
            let fallback_xchg = match fallback_ptr.addr() {
                addr if addr & (TRY_LOAD_FLAG | CONFIRM_FLAG) == 0 => continue,
                addr if addr == ptr::from_ref(&self.ptr).addr() | TRY_LOAD_FLAG => new_ptr.cast(),
                addr if addr == new_ptr.addr() | CONFIRM_FLAG => NULL,
                _ => continue,
            };
            // increment the refcount before in case fallback succeeds
            unsafe { A::incr_rc(new_ptr) };
            if (node.fallback)
                .compare_exchange(fallback_ptr, fallback_xchg, Release, Relaxed)
                .is_err()
            {
                unsafe { A::decr_rc(new_ptr) };
            }
        }
        old_arc
    }

    pub fn store(&self, arc: A) {
        drop(self.swap(arc));
    }
}

impl<A: ArcPtr + NonNullPtr, L: StaticBorrowList> AtomicArcPtr<A, L> {
    #[inline]
    pub fn load(&self) -> ArcPtrBorrow<A> {
        self.load_impl()
    }

    #[inline]
    pub fn load_if_outdated<'a>(&self, arc: &'a A) -> Result<&'a A, ArcPtrBorrow<A>> {
        self.load_if_outdated_impl(arc)
    }

    #[inline]
    pub fn load_cached<'a>(&self, cached: &'a mut A) -> &'a A {
        // using `load_if_outdated` doesn't give the exact same code
        let ptr = self.ptr.load(Relaxed);
        if ptr != A::as_ptr(cached) {
            *cached = self.load_with_ptr(ptr).into_owned();
        }
        cached
    }
}

impl<A: ArcPtr + NonNullPtr, L: StaticBorrowList> AtomicArcPtr<Option<A>, L> {
    #[inline]
    pub const fn null() -> Self {
        Self {
            ptr: AtomicPtr::new(NULL),
            _arc: PhantomData,
            _list: PhantomData,
        }
    }

    #[inline]
    pub fn load(&self) -> Option<ArcPtrBorrow<A>> {
        self.load_impl().into_opt()
    }

    #[inline]
    pub fn load_if_outdated<'a>(
        &self,
        arc: &'a Option<A>,
    ) -> Result<&'a Option<A>, Option<ArcPtrBorrow<A>>> {
        self.load_if_outdated_impl(arc)
            .map_err(ArcPtrBorrow::into_opt)
    }

    #[inline]
    pub fn load_cached<'a>(&self, cached: &'a mut Option<A>) -> Option<&'a A> {
        // using `load_if_outdated` doesn't give the exact same code
        let ptr = self.ptr.load(Relaxed);
        if ptr != Option::as_ptr(cached) {
            *cached = self.load_with_ptr(ptr).into_owned();
        }
        cached.as_ref()
    }
}

impl<A: ArcPtr, L: StaticBorrowList> Drop for AtomicArcPtr<A, L> {
    fn drop(&mut self) {
        let ptr = *self.ptr.get_mut();
        if !ptr.is_null() {
            self.swap_impl(NULL, ptr);
        }
    }
}

impl<A: ArcPtr + Default, L: StaticBorrowList> Default for AtomicArcPtr<A, L> {
    fn default() -> Self {
        Self::new(A::default())
    }
}

impl<A: ArcPtr, L: StaticBorrowList> From<A> for AtomicArcPtr<A, L> {
    fn from(value: A) -> Self {
        Self::new(value)
    }
}

impl<A: ArcPtr + NonNullPtr, L: StaticBorrowList> From<A> for AtomicArcPtr<Option<A>, L> {
    fn from(value: A) -> Self {
        Some(value).into()
    }
}

impl<T, L: StaticBorrowList> From<T> for AtomicArcPtr<Arc<T>, L> {
    fn from(value: T) -> Self {
        Arc::new(value).into()
    }
}

impl<T, L: StaticBorrowList> From<T> for AtomicArcPtr<Option<Arc<T>>, L> {
    fn from(value: T) -> Self {
        Some(Arc::new(value)).into()
    }
}

pub struct ArcPtrBorrow<A: ArcPtr> {
    borrow: Option<&'static Borrow>,
    arc: ManuallyDrop<A>,
}

impl<A: ArcPtr> ArcPtrBorrow<A> {
    #[inline(always)]
    fn new(ptr: *mut (), borrow: Option<&'static Borrow>) -> Self {
        let arc = ManuallyDrop::new(unsafe { A::from_ptr(ptr) });
        Self { borrow, arc }
    }

    #[inline]
    pub fn into_owned(self) -> A {
        if self.borrow.is_none() {
            return unsafe { ManuallyDrop::take(&mut ManuallyDrop::new(self).arc) };
        }
        self.clone()
    }
}

impl<A: ArcPtr + NonNullPtr> ArcPtrBorrow<Option<A>> {
    #[inline(always)]
    fn into_opt(self) -> Option<ArcPtrBorrow<A>> {
        let mut this = ManuallyDrop::new(self);
        Some(ArcPtrBorrow {
            borrow: this.borrow,
            arc: ManuallyDrop::new(unsafe { ManuallyDrop::take(&mut this.arc)? }),
        })
    }
}

impl<A: ArcPtr> Drop for ArcPtrBorrow<A> {
    #[inline]
    fn drop(&mut self) {
        let ptr = A::as_ptr(&self.arc);
        if (self.borrow).is_none_or(|b| b.compare_exchange(ptr, NULL, Relaxed, Relaxed).is_err()) {
            #[cold]
            #[inline(never)]
            fn drop_arc<A>(_: A) {}
            drop_arc(unsafe { ManuallyDrop::take(&mut self.arc) })
        }
    }
}

impl<A: ArcPtr> Deref for ArcPtrBorrow<A> {
    type Target = A;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.arc
    }
}

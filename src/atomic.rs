use alloc::sync::Arc;
use core::{
    fmt,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::Deref,
    ptr,
    sync::atomic::{
        AtomicPtr,
        Ordering::{Acquire, Relaxed, SeqCst},
    },
};

use crate::{
    NULL,
    arc::{ArcPtr, NonNullPtr},
    borrow_list::{Borrow, BorrowNodeRef, StaticBorrowList},
};

const PREPARE_LOAD_FLAG: usize = 0b01;
const CONFIRM_LOAD_FLAG: usize = 0b10;

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
    fn load_with_ptr(&self, mut ptr: *mut ()) -> ArcPtrBorrow<A> {
        if A::NULLABLE && ptr.is_null() {
            ptr = self.ptr.load(SeqCst);
            if ptr.is_null() {
                return ArcPtrBorrow::new(ptr, None);
            }
        }
        debug_assert!(!ptr.is_null());
        let node = L::thread_local_node();
        let borrow_idx = node.next_borrow_idx().get();
        let borrow = unsafe { node.borrows().get_unchecked(borrow_idx) };
        if borrow.load(Relaxed).is_null() {
            self.load_with_borrow(ptr, node, borrow, borrow_idx)
        } else {
            self.load_find_available_borrow(ptr, node)
        }
    }

    #[inline(always)]
    fn load_with_borrow(
        &self,
        ptr: *mut (),
        node: BorrowNodeRef,
        borrow: &'static Borrow,
        borrow_idx: usize,
    ) -> ArcPtrBorrow<A> {
        borrow.store(ptr, SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        if ptr != ptr_checked {
            return self.load_outdated(node, ptr, ptr_checked, borrow);
        }
        node.next_borrow_idx()
            .set((borrow_idx + 1) & node.borrow_idx_mask());
        ArcPtrBorrow::new(ptr_checked, Some(borrow))
    }

    #[cold]
    #[inline(never)]
    fn load_outdated(
        &self,
        node: BorrowNodeRef,
        ptr: *mut (),
        ptr_checked: *mut (),
        borrow: &'static Borrow,
    ) -> ArcPtrBorrow<A> {
        if A::NULLABLE && ptr_checked.is_null() {
            if (borrow.compare_exchange(ptr, NULL, SeqCst, Relaxed)).is_err() {
                unsafe { A::decr_rc(ptr) };
            }
            return ArcPtrBorrow::new(ptr_checked, None);
        }
        match borrow.compare_exchange(ptr, NULL, SeqCst, Relaxed) {
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
        let fallback = node.fallback();
        let prepare_ptr = ptr::from_ref(&self.ptr)
            .map_addr(|addr| addr | PREPARE_LOAD_FLAG)
            .cast_mut()
            .cast();
        fallback.store(prepare_ptr, SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        if A::NULLABLE && ptr_checked.is_null() {
            let ptr = fallback.swap(NULL, SeqCst);
            return ArcPtrBorrow::new(if ptr != prepare_ptr { ptr } else { ptr_checked }, None);
        }
        let confirm_ptr = ptr_checked.map_addr(|addr| addr | CONFIRM_LOAD_FLAG);
        // Failure ordering must be SeqCst for load to have a full SeqCst semantic
        if let Err(ptr) = fallback.compare_exchange(prepare_ptr, confirm_ptr, SeqCst, SeqCst) {
            fallback.store(NULL, SeqCst);
            return ArcPtrBorrow::new(ptr, None);
        }
        unsafe { A::incr_rc(ptr_checked) };
        if let Err(ptr) = fallback.compare_exchange(confirm_ptr, NULL, SeqCst, Acquire) {
            debug_assert!(ptr.is_null());
            unsafe { A::decr_rc(ptr_checked) };
        }
        ArcPtrBorrow::new(ptr_checked, None)
    }

    #[inline]
    pub fn load_owned(&self) -> A {
        self.load_impl().into_owned()
    }

    #[inline]
    fn load_if_outdated_impl<'a>(&self, arc: &'a A) -> Result<&'a A, ArcPtrBorrow<A>> {
        let ptr = self.ptr.load(Relaxed);
        if ptr == A::as_ptr(arc) {
            Ok(arc)
        } else {
            Err(self.load_with_ptr(ptr))
        }
    }

    pub fn swap(&self, arc: A) -> A {
        fn transfer_ownership<A: ArcPtr>(
            ptr: *mut (),
            op: impl FnOnce() -> Result<*mut (), *mut ()>,
        ) {
            unsafe { A::incr_rc(ptr) };
            if op().is_err() {
                unsafe { A::decr_rc(ptr) };
            }
        }
        let new_ptr = A::as_ptr(&arc);
        // store a clone in order to keep an owned arc, in case its ownership must be cloned
        let old_ptr = self.ptr.swap(A::into_ptr(arc.clone()), SeqCst);
        let old_arc = unsafe { A::from_ptr(old_ptr) };
        for node in L::static_list().nodes() {
            if !A::NULLABLE || !old_ptr.is_null() {
                for borrow in node.borrows().iter() {
                    if borrow.load(SeqCst) == old_ptr {
                        transfer_ownership::<A>(old_ptr, || {
                            borrow.compare_exchange(old_ptr, NULL, SeqCst, Relaxed)
                        });
                    }
                }
            }
            let fallback = node.fallback();
            let ptr = fallback.load(SeqCst);
            if ptr.addr() & (PREPARE_LOAD_FLAG | CONFIRM_LOAD_FLAG) == 0 {
                continue;
            } else if ptr.addr() == ptr::from_ref(&self.ptr).addr() | PREPARE_LOAD_FLAG {
                transfer_ownership::<A>(new_ptr, || {
                    match fallback.compare_exchange(ptr, new_ptr, SeqCst, Relaxed) {
                        Err(ptr) if ptr.addr() == old_ptr.addr() | CONFIRM_LOAD_FLAG => {
                            transfer_ownership::<A>(old_ptr, || {
                                fallback.compare_exchange(ptr, NULL, SeqCst, Relaxed)
                            });
                            Err(ptr)
                        }
                        res => res,
                    }
                });
            } else if ptr.addr() == old_ptr.addr() | CONFIRM_LOAD_FLAG {
                transfer_ownership::<A>(old_ptr, || {
                    fallback.compare_exchange(ptr, NULL, SeqCst, Relaxed)
                })
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
    pub const fn none() -> Self {
        Self {
            ptr: AtomicPtr::new(NULL),
            _arc: PhantomData,
            _list: PhantomData,
        }
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.ptr.load(Relaxed).is_null()
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
        if A::NULLABLE && ptr.is_null() {
            return;
        }
        let _arc = unsafe { A::from_ptr(ptr) };
        for node in L::static_list().nodes() {
            for borrow in node.borrows().iter() {
                if borrow.load(SeqCst) == ptr
                    && (borrow.compare_exchange(ptr, NULL, SeqCst, Relaxed)).is_ok()
                {
                    unsafe { A::incr_rc(ptr) }
                }
            }
        }
    }
}

impl<A: ArcPtr + Default, L: StaticBorrowList> Default for AtomicArcPtr<A, L> {
    fn default() -> Self {
        Self::new(A::default())
    }
}

impl<A: ArcPtr + fmt::Debug, L: StaticBorrowList> fmt::Debug for AtomicArcPtr<A, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AtomicArcPtr")
            .field(&self.load_impl())
            .finish()
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

impl<T, L: StaticBorrowList> From<Option<T>> for AtomicArcPtr<Option<Arc<T>>, L> {
    fn from(value: Option<T>) -> Self {
        value.map(Arc::new).into()
    }
}

#[derive(Debug)]
pub struct ArcPtrBorrow<A: ArcPtr> {
    arc: ManuallyDrop<A>,
    borrow: Option<&'static Borrow>,
}

impl<A: ArcPtr> ArcPtrBorrow<A> {
    #[inline(always)]
    fn new(ptr: *mut (), borrow: Option<&'static Borrow>) -> Self {
        let arc = ManuallyDrop::new(unsafe { A::from_ptr(ptr) });
        Self { arc, borrow }
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
        if (self.borrow).is_none_or(|b| b.compare_exchange(ptr, NULL, SeqCst, Relaxed).is_err()) {
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

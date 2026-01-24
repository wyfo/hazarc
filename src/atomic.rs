use alloc::sync::Arc;
use core::{
    fmt,
    marker::PhantomData,
    mem,
    mem::ManuallyDrop,
    ops::Deref,
    ptr,
    sync::atomic::{
        AtomicPtr, Ordering,
        Ordering::{Acquire, Relaxed, SeqCst},
    },
};

use crate::{
    NULL,
    arc::{ArcPtr, ArcRef, NonNullPtr},
    domain::{BorrowNodeRef, BorrowSlot, Domain},
};

const PREPARE_CLONE_FLAG: usize = 0b01;
const CONFIRM_CLONE_FLAG: usize = 0b10;

pub struct AtomicArcPtr<A: ArcPtr, D: Domain> {
    ptr: AtomicPtr<()>,
    _arc: PhantomData<A>,
    _list: PhantomData<D>,
}

impl<A: ArcPtr, D: Domain> AtomicArcPtr<A, D> {
    #[inline]
    pub fn new(arc: A) -> Self {
        Self {
            ptr: AtomicPtr::new(A::into_ptr(arc)),
            _arc: PhantomData,
            _list: PhantomData,
        }
    }

    #[inline]
    pub fn load(&self) -> ArcPtrBorrow<A> {
        self.load_impl(self.ptr.load(Relaxed))
    }

    #[inline(always)]
    fn load_impl(&self, mut ptr: *mut ()) -> ArcPtrBorrow<A> {
        if A::NULLABLE && ptr.is_null() {
            ptr = self.ptr.load(SeqCst);
            if ptr.is_null() {
                return ArcPtrBorrow::new(ptr, None);
            }
        }
        debug_assert!(!ptr.is_null());
        let node = D::thread_local_node();
        let slot_idx = node.next_borrow_slot_idx().get();
        let slot = unsafe { node.borrow_slots().get_unchecked(slot_idx) };
        if slot.load(Relaxed).is_null() {
            self.load_with_slot(ptr, node, slot, slot_idx)
        } else {
            self.load_find_available_slot(ptr, node)
        }
    }

    #[inline(always)]
    fn load_with_slot(
        &self,
        ptr: *mut (),
        node: BorrowNodeRef,
        slot: &'static BorrowSlot,
        slot_idx: usize,
    ) -> ArcPtrBorrow<A> {
        slot.store(ptr, SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        if ptr != ptr_checked {
            return self.load_outdated(node, ptr, ptr_checked, slot);
        }
        node.next_borrow_slot_idx()
            .set((slot_idx + 1) & node.borrow_slot_idx_mask());
        ArcPtrBorrow::new(ptr_checked, Some(slot))
    }

    #[cold]
    #[inline(never)]
    fn load_find_available_slot(&self, ptr: *mut (), node: BorrowNodeRef) -> ArcPtrBorrow<A> {
        match (node.borrow_slots().iter().enumerate())
            .find(|(_, borrow)| borrow.load(Relaxed).is_null())
        {
            Some((slot_idx, slot)) => self.load_with_slot(ptr, node, slot, slot_idx),
            None => self.load_clone(node),
        }
    }

    #[cold]
    #[inline(never)]
    fn load_outdated(
        &self,
        node: BorrowNodeRef,
        ptr: *mut (),
        ptr_checked: *mut (),
        borrow: &'static BorrowSlot,
    ) -> ArcPtrBorrow<A> {
        if A::NULLABLE && ptr_checked.is_null() {
            if (borrow.compare_exchange(ptr, NULL, SeqCst, Relaxed)).is_err() {
                unsafe { A::decr_rc(ptr) };
            }
            return ArcPtrBorrow::new(ptr_checked, None);
        }
        match borrow.compare_exchange(ptr, NULL, SeqCst, Relaxed) {
            Ok(_) => self.load_clone(node),
            Err(_) => ArcPtrBorrow::new(ptr, None),
        }
    }

    fn load_clone(&self, node: BorrowNodeRef) -> ArcPtrBorrow<A> {
        let clone_slot = node.clone_slot();
        let prepare_ptr = ptr::from_ref(&self.ptr)
            .map_addr(|addr| addr | PREPARE_CLONE_FLAG)
            .cast_mut()
            .cast();
        clone_slot.store(prepare_ptr, SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        if A::NULLABLE && ptr_checked.is_null() {
            let ptr = clone_slot.swap(NULL, SeqCst);
            return ArcPtrBorrow::new(if ptr != prepare_ptr { ptr } else { ptr_checked }, None);
        }
        let confirm_ptr = ptr_checked.map_addr(|addr| addr | CONFIRM_CLONE_FLAG);
        // Failure ordering must be SeqCst for load to have a full SeqCst semantic
        if let Err(ptr) = clone_slot.compare_exchange(prepare_ptr, confirm_ptr, SeqCst, SeqCst) {
            clone_slot.store(NULL, SeqCst);
            return ArcPtrBorrow::new(ptr, None);
        }
        unsafe { A::incr_rc(ptr_checked) };
        if let Err(ptr) = clone_slot.compare_exchange(confirm_ptr, NULL, SeqCst, Acquire) {
            debug_assert!(ptr.is_null());
            unsafe { A::decr_rc(ptr_checked) };
        }
        ArcPtrBorrow::new(ptr_checked, None)
    }

    #[inline]
    pub fn load_owned(&self) -> A {
        self.load().into_owned()
    }

    #[cold]
    fn load_impl_cold(&self, ptr: *mut ()) -> ArcPtrBorrow<A> {
        self.load_impl(ptr)
    }

    #[inline(always)]
    fn load_if_outdated_impl<'a>(
        &self,
        arc: &'a A,
        ordering: Ordering,
    ) -> Result<&'a A, ArcPtrBorrow<A>> {
        let ptr = self.ptr.load(ordering);
        if ptr == A::as_ptr(arc) {
            Ok(arc)
        } else {
            Err(self.load_impl_cold(ptr))
        }
    }

    #[inline]
    pub fn load_if_outdated<'a>(&self, arc: &'a A) -> Result<&'a A, ArcPtrBorrow<A>> {
        self.load_if_outdated_impl(arc, SeqCst)
    }

    #[inline]
    pub fn load_if_outdated_relaxed<'a>(&self, arc: &'a A) -> Result<&'a A, ArcPtrBorrow<A>> {
        self.load_if_outdated_impl(arc, Relaxed)
    }

    #[inline]
    pub fn load_cached_impl<'a>(&self, cached: &'a mut A, ordering: Ordering) -> &'a A {
        let ptr = self.ptr.load(ordering);
        if ptr != A::as_ptr(cached) {
            self.reload_cache(ptr, cached);
        }
        cached
    }

    #[cold]
    #[inline(never)]
    fn reload_cache(&self, ptr: *mut (), cached: &mut A) {
        *cached = self.load_impl(ptr).into_owned();
    }

    #[inline]
    pub fn load_cached<'a>(&self, cached: &'a mut A) -> &'a A {
        self.load_cached_impl(cached, SeqCst)
    }

    #[inline]
    pub fn load_cached_relaxed<'a>(&self, cached: &'a mut A) -> &'a A {
        self.load_cached_impl(cached, Relaxed)
    }

    pub fn swap(&self, arc: A) -> A {
        // store a clone in order to keep an owned arc, in case its ownership must be transferred
        let old_ptr = self.ptr.swap(A::into_ptr(arc.clone()), SeqCst);
        self.swap_impl(old_ptr, Some(arc))
    }

    fn swap_impl(&self, old_ptr: *mut (), new: Option<A>) -> A {
        fn transfer_ownership<A: ArcPtr>(
            ptr: *mut (),
            op: impl FnOnce() -> Result<*mut (), *mut ()>,
        ) {
            unsafe { A::incr_rc(ptr) };
            if op().is_err() {
                unsafe { A::decr_rc(ptr) };
            }
        }
        let new_ptr = new.as_ref().map(A::as_ptr);
        let old_arc = unsafe { A::from_ptr(old_ptr) };
        for node in D::static_list().nodes() {
            if !A::NULLABLE || !old_ptr.is_null() {
                for slot in node.borrow_slots().iter() {
                    if slot.load(SeqCst) == old_ptr {
                        transfer_ownership::<A>(old_ptr, || {
                            slot.compare_exchange(old_ptr, NULL, SeqCst, Relaxed)
                        });
                    }
                }
            }
            let Some(new_ptr) = new_ptr else {
                continue;
            };
            let clone_slot = node.clone_slot();
            let ptr = clone_slot.load(SeqCst);
            if ptr.addr() & (PREPARE_CLONE_FLAG | CONFIRM_CLONE_FLAG) == 0 {
                continue;
            } else if ptr.addr() == ptr::from_ref(&self.ptr).addr() | PREPARE_CLONE_FLAG {
                transfer_ownership::<A>(new_ptr, || {
                    match clone_slot.compare_exchange(ptr, new_ptr, SeqCst, Relaxed) {
                        Err(ptr) if ptr.addr() == old_ptr.addr() | CONFIRM_CLONE_FLAG => {
                            transfer_ownership::<A>(old_ptr, || {
                                clone_slot.compare_exchange(ptr, NULL, SeqCst, Relaxed)
                            });
                            Err(ptr)
                        }
                        res => res,
                    }
                });
            } else if ptr.addr() == old_ptr.addr() | CONFIRM_CLONE_FLAG {
                transfer_ownership::<A>(old_ptr, || {
                    clone_slot.compare_exchange(ptr, NULL, SeqCst, Relaxed)
                })
            }
        }
        old_arc
    }

    pub fn store(&self, arc: A) {
        drop(self.swap(arc));
    }

    pub fn compare_exchange<C: ArcRef<A>>(&self, current: C, new: A) -> Result<A, ArcPtrBorrow<A>> {
        // store a clone in order to keep an owned arc, in case its ownership must be transferred
        let new_clone = A::into_ptr(new.clone());
        match (self.ptr).compare_exchange(C::as_ptr(current), new_clone, SeqCst, Relaxed) {
            Ok(old_ptr) => Ok(self.swap_impl(old_ptr, Some(new))),
            Err(old_ptr) => {
                unsafe { A::decr_rc(new_clone) };
                Err(self.load_impl(old_ptr))
            }
        }
    }

    pub fn fetch_update<F: FnMut(&A) -> Option<R>, R: Into<A>>(
        &self,
        mut f: F,
    ) -> Result<A, ArcPtrBorrow<A>> {
        let mut current = self.load();
        while let Some(new) = f(&current) {
            match self.compare_exchange(&*current, new.into()) {
                Ok(old_arc) => return Ok(old_arc),
                Err(old_arc) => current = old_arc,
            }
        }
        Err(current)
    }
}

impl<A: ArcPtr + NonNullPtr, D: Domain> AtomicArcPtr<Option<A>, D> {
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
    pub fn load_relaxed(&self) -> ArcPtrBorrow<Option<A>> {
        let ptr = self.ptr.load(Relaxed);
        if ptr.is_null() {
            return None::<A>.into();
        }
        self.load_impl(ptr)
    }
}

impl<A: ArcPtr, D: Domain> Drop for AtomicArcPtr<A, D> {
    fn drop(&mut self) {
        let ptr = *self.ptr.get_mut();
        if !A::NULLABLE || !ptr.is_null() {
            self.swap_impl(ptr, None);
        }
    }
}

impl<A: ArcPtr + Default, D: Domain> Default for AtomicArcPtr<A, D> {
    fn default() -> Self {
        Self::new(A::default())
    }
}

impl<A: ArcPtr + fmt::Debug, D: Domain> fmt::Debug for AtomicArcPtr<A, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AtomicArcPtr").field(&self.load()).finish()
    }
}

impl<A: ArcPtr, D: Domain> From<A> for AtomicArcPtr<A, D> {
    fn from(value: A) -> Self {
        Self::new(value)
    }
}

impl<A: ArcPtr + NonNullPtr, D: Domain> From<A> for AtomicArcPtr<Option<A>, D> {
    fn from(value: A) -> Self {
        Some(value).into()
    }
}

impl<T, D: Domain> From<T> for AtomicArcPtr<Arc<T>, D> {
    fn from(value: T) -> Self {
        Arc::new(value).into()
    }
}

impl<T, D: Domain> From<T> for AtomicArcPtr<Option<Arc<T>>, D> {
    fn from(value: T) -> Self {
        Some(Arc::new(value)).into()
    }
}

impl<T, D: Domain> From<Option<T>> for AtomicArcPtr<Option<Arc<T>>, D> {
    fn from(value: Option<T>) -> Self {
        value.map(Arc::new).into()
    }
}

#[derive(Debug)]
pub struct ArcPtrBorrow<A: ArcPtr> {
    arc: ManuallyDrop<A>,
    borrow: Option<&'static BorrowSlot>,
}

impl<A: ArcPtr> ArcPtrBorrow<A> {
    #[inline(always)]
    fn new(ptr: *mut (), borrow: Option<&'static BorrowSlot>) -> Self {
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
    pub fn transpose(self) -> Option<ArcPtrBorrow<A>> {
        let this = ManuallyDrop::new(self);
        Some(ArcPtrBorrow::new(
            A::as_ptr(this.arc.as_ref()?),
            this.borrow,
        ))
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

// NonNullPtr bound to avoid collision with `Option::as_ref`
impl<A: ArcPtr + NonNullPtr> AsRef<A> for ArcPtrBorrow<A> {
    #[inline]
    fn as_ref(&self) -> &A {
        self
    }
}

impl<A: ArcPtr> From<A> for ArcPtrBorrow<A> {
    #[inline]
    fn from(value: A) -> Self {
        Self {
            arc: ManuallyDrop::new(value),
            borrow: None,
        }
    }
}

impl<A: ArcPtr + NonNullPtr> From<Option<ArcPtrBorrow<A>>> for ArcPtrBorrow<Option<A>> {
    #[inline]
    fn from(value: Option<ArcPtrBorrow<A>>) -> Self {
        match value.map(ManuallyDrop::new) {
            Some(a) => Self::new(A::as_ptr(&a.arc), a.borrow),
            None => Self::new(NULL, None),
        }
    }
}

#[repr(transparent)]
pub struct AtomicOptionArcPtr<A: ArcPtr + NonNullPtr, D: Domain>(AtomicArcPtr<Option<A>, D>);

impl<A: ArcPtr + NonNullPtr, D: Domain> AtomicOptionArcPtr<A, D> {
    #[inline]
    pub fn new(arc: Option<A>) -> Self {
        Self(AtomicArcPtr::new(arc))
    }

    #[inline]
    pub fn inner(&self) -> &AtomicArcPtr<Option<A>, D> {
        &self.0
    }

    #[inline]
    pub fn into_inner(self) -> AtomicArcPtr<Option<A>, D> {
        self.0
    }

    #[inline]
    pub const fn none() -> Self {
        Self(AtomicArcPtr::none())
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    #[inline]
    pub fn load_relaxed(&self) -> Option<ArcPtrBorrow<A>> {
        self.0.load_relaxed().transpose()
    }

    #[inline]
    pub fn load(&self) -> Option<ArcPtrBorrow<A>> {
        self.0.load().transpose()
    }

    #[inline]
    pub fn load_owned(&self) -> Option<A> {
        self.0.load_owned()
    }

    #[inline]
    pub fn load_if_outdated<'a>(
        &self,
        arc: &'a Option<A>,
    ) -> Result<&'a Option<A>, Option<ArcPtrBorrow<A>>> {
        self.0
            .load_if_outdated(arc)
            .map_err(ArcPtrBorrow::transpose)
    }

    #[inline]
    pub fn load_if_outdated_relaxed<'a>(
        &self,
        arc: &'a Option<A>,
    ) -> Result<&'a Option<A>, Option<ArcPtrBorrow<A>>> {
        self.0
            .load_if_outdated_relaxed(arc)
            .map_err(ArcPtrBorrow::transpose)
    }

    #[inline]
    pub fn load_cached<'a>(&self, cached: &'a mut Option<A>) -> Option<&'a A> {
        self.0.load_cached(cached).as_ref()
    }

    #[inline]
    pub fn load_cached_relaxed<'a>(&self, cached: &'a mut Option<A>) -> Option<&'a A> {
        self.0.load_cached_relaxed(cached).as_ref()
    }

    pub fn swap(&self, new: Option<A>) -> Option<A> {
        self.0.swap(new)
    }

    pub fn store(&self, new: Option<A>) {
        self.0.store(new);
    }

    pub fn compare_exchange<C: ArcRef<Option<A>>>(
        &self,
        current: C,
        new: Option<A>,
    ) -> Result<Option<A>, Option<ArcPtrBorrow<A>>> {
        self.0
            .compare_exchange(current, new)
            .map_err(ArcPtrBorrow::transpose)
    }

    pub fn fetch_update<F: FnMut(Option<&A>) -> Option<R>, R: Into<Option<A>>>(
        &self,
        mut f: F,
    ) -> Result<Option<A>, Option<ArcPtrBorrow<A>>> {
        self.0
            .fetch_update(|arc| f(arc.as_ref()))
            .map_err(ArcPtrBorrow::transpose)
    }
}
impl<A: ArcPtr + NonNullPtr, D: Domain> Default for AtomicOptionArcPtr<A, D> {
    fn default() -> Self {
        Self::none()
    }
}

impl<A: ArcPtr + NonNullPtr + fmt::Debug, D: Domain> fmt::Debug for AtomicOptionArcPtr<A, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AtomicOptionArcPtr").field(&self.0).finish()
    }
}

impl<T, A: ArcPtr + NonNullPtr, D: Domain> From<T> for AtomicOptionArcPtr<A, D>
where
    AtomicArcPtr<Option<A>, D>: From<T>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl<'a, A: ArcPtr + NonNullPtr, D: Domain> From<&'a AtomicArcPtr<Option<A>, D>>
    for &'a AtomicOptionArcPtr<A, D>
{
    fn from(value: &'a AtomicArcPtr<Option<A>, D>) -> Self {
        unsafe { mem::transmute::<&'a AtomicArcPtr<Option<A>, D>, Self>(value) }
    }
}

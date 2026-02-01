use alloc::sync::Arc;
use core::{
    fmt, hint,
    marker::PhantomData,
    mem,
    mem::ManuallyDrop,
    ops::Deref,
    sync::atomic::{
        AtomicPtr,
        Ordering::{Acquire, Relaxed, SeqCst},
    },
};

#[allow(unused_imports)]
use crate::msrv::{OptionExt, StrictProvenance};
use crate::{
    arc::{ArcPtr, ArcRef, NonNullPtr},
    domain::{BorrowNodeRef, BorrowSlot, Domain},
    msrv::ptr,
    write_policy::{Concurrent, WritePolicy},
    NULL,
};

const PREPARE_CLONE_FLAG: usize = 0b01;
const CONFIRM_CLONE_FLAG: usize = 0b10;
const GENERATION_INCR: usize = PREPARE_CLONE_FLAG + 1;
const MAX_GENERATION: usize = !PREPARE_CLONE_FLAG;

pub struct AtomicArcPtr<A: ArcPtr, D: Domain, W: WritePolicy> {
    ptr: AtomicPtr<()>,
    _arc: PhantomData<A>,
    _domain: PhantomData<D>,
    _write_policy: PhantomData<W>,
}

impl<A: ArcPtr, D: Domain, W: WritePolicy> AtomicArcPtr<A, D, W> {
    #[inline]
    pub fn new(arc: A) -> Self {
        Self {
            ptr: AtomicPtr::new(A::into_ptr(arc)),
            _arc: PhantomData,
            _domain: PhantomData,
            _write_policy: PhantomData,
        }
    }

    #[inline(always)]
    fn first_load(&self) -> *mut () {
        self.ptr.load(if A::NULLABLE { SeqCst } else { Relaxed })
    }

    #[inline]
    pub fn load(&self) -> ArcPtrBorrow<A> {
        self.load_impl(self.first_load())
    }

    #[inline(always)]
    fn load_impl(&self, ptr: *mut ()) -> ArcPtrBorrow<A> {
        if A::NULLABLE && ptr.is_null() {
            return ArcPtrBorrow::new(NULL, None);
        }
        debug_assert!(!ptr.is_null());
        let node = D::get_or_insert_thread_local_node();
        let slot_idx = match D::BORROW_SLOT_COUNT {
            0 => return self.load_clone(node),
            1 => 0,
            _ => node.next_borrow_slot_idx().get(),
        };
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
        node: BorrowNodeRef<D>,
        slot: &'static BorrowSlot,
        slot_idx: usize,
    ) -> ArcPtrBorrow<A> {
        slot.store(ptr, SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        if ptr != ptr_checked {
            return self.load_outdated(node, ptr, ptr_checked, slot);
        }
        if D::BORROW_SLOT_COUNT > 1 {
            // The assertion is already known by compiler in `load_impl` with `get_unchecked`,
            // but it has to be repeated here to be taken in account for the modulo when borrow
            // slot count is not a multiple of 2
            if slot_idx >= D::BORROW_SLOT_COUNT {
                unsafe { hint::unreachable_unchecked() }; // MSRV 1.81
            }
            node.next_borrow_slot_idx()
                .set((slot_idx + 1) % D::BORROW_SLOT_COUNT);
        }
        ArcPtrBorrow::new(ptr_checked, Some(slot))
    }

    #[cold]
    #[inline(never)]
    fn load_find_available_slot(&self, ptr: *mut (), node: BorrowNodeRef<D>) -> ArcPtrBorrow<A> {
        match (node.borrow_slots().iter().enumerate())
            .find(|(_, slot)| slot.load(Relaxed).is_null())
        {
            Some((slot_idx, slot)) => self.load_with_slot(ptr, node, slot, slot_idx),
            None => self.load_clone(node),
        }
    }

    #[cold]
    #[inline(never)]
    fn load_outdated(
        &self,
        node: BorrowNodeRef<D>,
        ptr: *mut (),
        ptr_checked: *mut (),
        slot: &'static BorrowSlot,
    ) -> ArcPtrBorrow<A> {
        if A::NULLABLE && ptr_checked.is_null() {
            if let Err(p) = slot.compare_exchange(ptr, NULL, SeqCst, Relaxed) {
                debug_assert!(p.is_null());
                unsafe { A::decr_rc(ptr) };
            }
            ArcPtrBorrow::new(NULL, None)
        } else if let Err(p) = slot.compare_exchange(ptr, NULL, SeqCst, Relaxed) {
            debug_assert!(p.is_null());
            ArcPtrBorrow::new(ptr, None)
        } else {
            self.load_clone(node)
        }
    }

    #[allow(unstable_name_collisions)]
    fn load_clone(&self, node: BorrowNodeRef<D>) -> ArcPtrBorrow<A> {
        let clone_slot = node.clone_slot();
        let self_ptr = ptr::from_ref(&self.ptr).cast_mut().cast();
        let prepare_ptr = if W::CONCURRENT {
            node.atomic_arc_slot().store(self_ptr, Relaxed);
            let generation = node.clone_generation().get();
            node.clone_generation()
                .set(generation.wrapping_add(GENERATION_INCR));
            if generation == MAX_GENERATION {
                D::reset_thread_local_node();
            }
            ptr::without_provenance_mut(generation | PREPARE_CLONE_FLAG)
        } else {
            self_ptr.map_addr(|addr| addr | PREPARE_CLONE_FLAG)
        };
        clone_slot.store(prepare_ptr, SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        if A::NULLABLE && ptr_checked.is_null() {
            let ptr = clone_slot.swap(NULL, SeqCst);
            if ptr != prepare_ptr {
                unsafe { A::decr_rc(ptr) };
            }
            return ArcPtrBorrow::new(NULL, None);
        }
        let confirm_ptr = (ptr_checked).map_addr(|addr| addr | CONFIRM_CLONE_FLAG);
        // Failure ordering must be SeqCst for load to have a full SeqCst semantic
        if let Err(ptr) = clone_slot.compare_exchange(prepare_ptr, confirm_ptr, SeqCst, SeqCst) {
            clone_slot.store(NULL, SeqCst);
            return ArcPtrBorrow::new(ptr, None);
        }
        unsafe { A::incr_rc(ptr_checked) };
        if let Err(p) = (node.clone_slot()).compare_exchange(confirm_ptr, NULL, SeqCst, Acquire) {
            debug_assert!(p.is_null());
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

    #[inline]
    pub fn load_if_outdated<'a>(&self, arc: &'a A) -> Result<&'a A, ArcPtrBorrow<A>> {
        let ptr = self.first_load();
        if ptr == A::as_ptr(arc) {
            Ok(arc)
        } else {
            Err(self.load_impl_cold(ptr))
        }
    }

    #[cold]
    #[inline(never)]
    fn reload_cache(&self, ptr: *mut (), cached: &mut A) {
        *cached = self.load_impl(ptr).into_owned();
    }

    #[inline]
    pub fn load_cached<'a>(&self, cached: &'a mut A) -> &'a A {
        let ptr = self.first_load();
        if ptr != A::as_ptr(cached) {
            self.reload_cache(ptr, cached);
        }
        cached
    }

    pub fn swap(&self, arc: A) -> A {
        // store a clone in order to keep an owned arc, in case its ownership must be transferred
        let old_ptr = self.ptr.swap(A::into_ptr(arc.clone()), SeqCst);
        self.swap_impl(old_ptr, Some(arc))
    }

    #[allow(unstable_name_collisions)]
    fn swap_impl(&self, old_ptr: *mut (), mut new: Option<A>) -> A {
        fn transfer_ownership<A: ArcPtr>(
            ptr: *mut (),
            op: impl FnOnce() -> Result<*mut (), *mut ()>,
        ) -> Result<*mut (), *mut ()> {
            unsafe { A::incr_rc(ptr) };
            let res = op();
            if res.is_err() {
                unsafe { A::decr_rc(ptr) };
            }
            res
        }
        let old_arc = unsafe { A::from_ptr(old_ptr) };
        for node in D::static_list().nodes() {
            #[cfg(any(
                not(target_pointer_width = "64"),
                hazarc_force_active_writer_count_64bit
            ))]
            let _guard = W::CONCURRENT.then(|| node.writer_guard());
            if !A::NULLABLE || !old_ptr.is_null() {
                for slot in node.borrow_slots().iter() {
                    if slot.load(SeqCst) == old_ptr {
                        let _ = transfer_ownership::<A>(old_ptr, || {
                            // Acquire failure so borrow happens before
                            slot.compare_exchange(old_ptr, NULL, SeqCst, Acquire)
                        });
                    }
                }
            }
            let Some(mut new_ptr) = new.as_ref().map(A::as_ptr) else {
                continue;
            };
            let clone_slot = node.clone_slot();
            let mut clone_ptr = clone_slot.load(SeqCst);
            if clone_ptr.addr() & (PREPARE_CLONE_FLAG | CONFIRM_CLONE_FLAG) == 0 {
                continue;
            }
            if clone_ptr.addr() & PREPARE_CLONE_FLAG != 0
                && self.is_same_atomic_arc(node, &mut clone_ptr)
            {
                if W::CONCURRENT {
                    // Reload the arc if it is outdated to avoid non-monotonic loads,
                    // as this swap execution could be late, and a previous load of
                    // this node's thread could have loaded the value arc of a subsequent swap
                    let ptr_checked = self.ptr.load(SeqCst);
                    if ptr_checked != new_ptr {
                        let arc = self.load_impl(ptr_checked).into_owned();
                        new_ptr = A::as_ptr(&arc);
                        new = Some(arc);
                    }
                }
                if let Err(p) = transfer_ownership::<A>(new_ptr, || {
                    clone_slot.compare_exchange(clone_ptr, new_ptr, SeqCst, Relaxed)
                }) {
                    clone_ptr = p;
                }
            }
            if clone_ptr.addr() == old_ptr.addr() | CONFIRM_CLONE_FLAG {
                let _ = transfer_ownership::<A>(old_ptr, || {
                    clone_slot.compare_exchange(clone_ptr, NULL, SeqCst, Relaxed)
                });
            }
        }
        old_arc
    }

    #[allow(unstable_name_collisions)]
    fn is_same_atomic_arc(&self, node: BorrowNodeRef<D>, clone_ptr: &mut *mut ()) -> bool {
        let self_ptr = ptr::from_ref(&self.ptr).cast_mut().cast();
        if W::CONCURRENT {
            node.atomic_arc_slot().load(Relaxed) == self_ptr && {
                let prev_clone_ptr = *clone_ptr;
                *clone_ptr = node.clone_slot().load(SeqCst);
                *clone_ptr == prev_clone_ptr
            }
        } else {
            clone_ptr.addr() == self_ptr.addr() | PREPARE_CLONE_FLAG
        }
    }

    pub fn store(&self, arc: A) {
        drop(self.swap(arc));
    }

    /// # Safety
    ///
    /// `self` must not be reused after.
    #[inline(always)]
    unsafe fn take_owned(&mut self) -> A {
        let ptr = *self.ptr.get_mut();
        if A::NULLABLE && ptr.is_null() {
            return unsafe { A::from_ptr(NULL) };
        }
        self.swap_impl(ptr, None)
    }

    #[inline]
    pub fn into_owned(self) -> A {
        // SAFETY: self is not reused after
        unsafe { ManuallyDrop::new(self).take_owned() }
    }
}

impl<A: ArcPtr, D: Domain> AtomicArcPtr<A, D, Concurrent> {
    pub fn compare_exchange<C: ArcRef<A>>(&self, current: C, new: A) -> Result<A, ArcPtrBorrow<A>> {
        // store a clone in order to keep an owned arc, in case its ownership must be transferred
        let new_clone = A::into_ptr(new.clone());
        match (self.ptr).compare_exchange(C::as_ptr(current), new_clone, SeqCst, Acquire) {
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

impl<A: ArcPtr + NonNullPtr, D: Domain, W: WritePolicy> AtomicArcPtr<Option<A>, D, W> {
    #[inline]
    pub const fn none() -> Self {
        Self {
            ptr: AtomicPtr::new(NULL),
            _arc: PhantomData,
            _domain: PhantomData,
            _write_policy: PhantomData,
        }
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.ptr.load(SeqCst).is_null()
    }
}

impl<A: ArcPtr, D: Domain, W: WritePolicy> Drop for AtomicArcPtr<A, D, W> {
    fn drop(&mut self) {
        // SAFETY: self is not reused after
        drop(unsafe { self.take_owned() });
    }
}

impl<A: ArcPtr + Default, D: Domain, W: WritePolicy> Default for AtomicArcPtr<A, D, W> {
    fn default() -> Self {
        Self::new(A::default())
    }
}

impl<A: ArcPtr + fmt::Debug, D: Domain, W: WritePolicy> fmt::Debug for AtomicArcPtr<A, D, W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AtomicArcPtr").field(&*self.load()).finish()
    }
}

impl<A: ArcPtr, D: Domain, W: WritePolicy> From<A> for AtomicArcPtr<A, D, W> {
    fn from(value: A) -> Self {
        Self::new(value)
    }
}

impl<A: ArcPtr + NonNullPtr, D: Domain, W: WritePolicy> From<A> for AtomicArcPtr<Option<A>, D, W> {
    fn from(value: A) -> Self {
        Some(value).into()
    }
}

impl<T, D: Domain, W: WritePolicy> From<T> for AtomicArcPtr<Arc<T>, D, W> {
    fn from(value: T) -> Self {
        Arc::new(value).into()
    }
}

impl<T, D: Domain, W: WritePolicy> From<T> for AtomicArcPtr<Option<Arc<T>>, D, W> {
    fn from(value: T) -> Self {
        Some(Arc::new(value)).into()
    }
}

impl<T, D: Domain, W: WritePolicy> From<Option<T>> for AtomicArcPtr<Option<Arc<T>>, D, W> {
    fn from(value: Option<T>) -> Self {
        value.map(Arc::new).into()
    }
}

#[must_use]
#[derive(Debug)]
pub struct ArcPtrBorrow<A: ArcPtr> {
    arc: ManuallyDrop<A>,
    slot: Option<&'static BorrowSlot>,
}

impl<A: ArcPtr> ArcPtrBorrow<A> {
    #[inline(always)]
    pub(crate) fn new(ptr: *mut (), slot: Option<&'static BorrowSlot>) -> Self {
        let arc = ManuallyDrop::new(unsafe { A::from_ptr(ptr) });
        Self { arc, slot }
    }

    #[inline]
    pub fn into_owned(self) -> A {
        if self.slot.is_none() {
            return unsafe { ManuallyDrop::take(&mut ManuallyDrop::new(self).arc) };
        }
        self.clone()
    }
}

impl<A: ArcPtr + NonNullPtr> ArcPtrBorrow<Option<A>> {
    #[inline(always)]
    pub fn transpose(self) -> Option<ArcPtrBorrow<A>> {
        let this = ManuallyDrop::new(self);
        Some(ArcPtrBorrow::new(A::as_ptr(this.arc.as_ref()?), this.slot))
    }
}

impl<A: ArcPtr> Drop for ArcPtrBorrow<A> {
    #[inline]
    fn drop(&mut self) {
        let ptr = A::as_ptr(&self.arc);
        // Acquire failure so other successfully released borrow which happens before the release
        // in swap happens before this one
        if (self.slot).is_none_or(|slot| slot.compare_exchange(ptr, NULL, SeqCst, Acquire).is_err())
        {
            #[cold]
            #[inline(never)]
            fn drop_arc<A>(_: A) {}
            drop_arc(unsafe { ManuallyDrop::take(&mut self.arc) });
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

impl<A: ArcPtr + fmt::Display> fmt::Display for ArcPtrBorrow<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<A: ArcPtr> From<A> for ArcPtrBorrow<A> {
    #[inline]
    fn from(value: A) -> Self {
        Self {
            arc: ManuallyDrop::new(value),
            slot: None,
        }
    }
}

impl<A: ArcPtr + NonNullPtr> From<Option<ArcPtrBorrow<A>>> for ArcPtrBorrow<Option<A>> {
    #[inline]
    fn from(value: Option<ArcPtrBorrow<A>>) -> Self {
        match value.map(ManuallyDrop::new) {
            Some(a) => Self::new(A::as_ptr(&a.arc), a.slot),
            None => Self::new(NULL, None),
        }
    }
}

#[repr(transparent)]
pub struct AtomicOptionArcPtr<A: ArcPtr + NonNullPtr, D: Domain, W: WritePolicy>(
    AtomicArcPtr<Option<A>, D, W>,
);

impl<A: ArcPtr + NonNullPtr, D: Domain, W: WritePolicy> AtomicOptionArcPtr<A, D, W> {
    #[inline]
    pub fn new(arc: Option<A>) -> Self {
        Self(AtomicArcPtr::new(arc))
    }

    #[inline]
    pub fn inner(&self) -> &AtomicArcPtr<Option<A>, D, W> {
        &self.0
    }

    #[inline]
    pub fn into_inner(self) -> AtomicArcPtr<Option<A>, D, W> {
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
    pub fn load(&self) -> Option<ArcPtrBorrow<A>> {
        self.0.load().transpose()
    }

    #[inline]
    pub fn load_owned(&self) -> Option<A> {
        self.0.load_owned()
    }

    #[inline]
    pub fn into_owned(self) -> Option<A> {
        self.0.into_owned()
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
    pub fn load_cached<'a>(&self, cached: &'a mut Option<A>) -> Option<&'a A> {
        self.0.load_cached(cached).as_ref()
    }

    pub fn swap(&self, new: Option<A>) -> Option<A> {
        self.0.swap(new)
    }

    pub fn store(&self, new: Option<A>) {
        self.0.store(new);
    }
}

impl<A: ArcPtr + NonNullPtr, D: Domain> AtomicOptionArcPtr<A, D, Concurrent> {
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
impl<A: ArcPtr + NonNullPtr, D: Domain, W: WritePolicy> Default for AtomicOptionArcPtr<A, D, W> {
    fn default() -> Self {
        Self::none()
    }
}

impl<A: ArcPtr + NonNullPtr + fmt::Debug, D: Domain, W: WritePolicy> fmt::Debug
    for AtomicOptionArcPtr<A, D, W>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AtomicOptionArcPtr").field(&self.0).finish()
    }
}

impl<T, A: ArcPtr + NonNullPtr, D: Domain, W: WritePolicy> From<T> for AtomicOptionArcPtr<A, D, W>
where
    AtomicArcPtr<Option<A>, D, W>: From<T>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl<'a, A: ArcPtr + NonNullPtr, D: Domain, W: WritePolicy> From<&'a AtomicArcPtr<Option<A>, D, W>>
    for &'a AtomicOptionArcPtr<A, D, W>
{
    fn from(value: &'a AtomicArcPtr<Option<A>, D, W>) -> Self {
        unsafe { mem::transmute::<&'a AtomicArcPtr<Option<A>, D, W>, Self>(value) }
    }
}

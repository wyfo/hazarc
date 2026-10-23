//! Generic atomic storage for `Arc`-like pointers.

#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use core::{fmt, marker::PhantomData, mem, mem::ManuallyDrop, ops::Deref};

#[allow(unused_imports)]
use crate::msrv::{OptionExt, StrictProvenance};
use crate::{
    arc::{ArcPtr, ArcPtrExt, ArcRef, DropVTable, NonNullArcPtr},
    domain::{Domain, DomainExt, DomainNodeRef, PrivateDomainExt, SlotRef, SEQ_CST_LOAD_IS_FREE},
    loom::{
        sync::atomic::{
            fence, AtomicPtr, Ordering,
            Ordering::{AcqRel, Acquire, Relaxed, Release, SeqCst},
        },
        AtomicPtrExt,
    },
    msrv::{hint, ptr},
    write_policy::{Concurrent, WritePolicy},
    NULL,
};

// Slot tags, stored in the 2 LSBs (`ArcPtr` pointers are aligned to 4).
pub(crate) const TAG_MASK: usize = 0b11;
/// A borrowed (validated or not) Arc pointer.
#[allow(dead_code)]
const BORROWED: usize = 0b00;
/// The ownership of a borrowed pointer has been transferred by a writer, with its [`DropVTable`].
const UPGRADED: usize = 0b01;
/// A slow-path load is pending, with the loaded `AtomicArc` address (`Serialized`)
/// or a generation (`Concurrent`).
const LOADING: usize = 0b10;
/// An owned Arc pointer provided by a helping writer to a concurrent loading.
const OWNED: usize = 0b11;

pub(crate) trait TaggedPtr: Sized {
    fn tag(self) -> usize;
    fn with_tag(self, tag: usize) -> Self;
    fn without_tag(self, tag: usize) -> Self;
}
impl TaggedPtr for *mut () {
    #[inline(always)]
    fn tag(self) -> usize {
        self.addr() & TAG_MASK
    }
    #[inline(always)]
    fn with_tag(self, tag: usize) -> Self {
        debug_assert_eq!(self.tag(), 0);
        self.map_addr(|addr| addr + tag)
    }
    #[inline(always)]
    fn without_tag(self, tag: usize) -> Self {
        debug_assert_eq!(self.tag(), tag);
        self.map_addr(|addr| addr - tag)
    }
}

/// An atomic storage for generic `Arc`-like pointers.
///
/// See [crate] documentation.
pub struct AtomicArcPtr<A: ArcPtr, D: Domain, W: WritePolicy> {
    ptr: AtomicPtr<()>,
    _arc: PhantomData<A>,
    _domain: PhantomData<D>,
    _write_policy: PhantomData<W>,
}

impl<A: ArcPtr, D: Domain, W: WritePolicy> AtomicArcPtr<A, D, W> {
    /// Constructs a new Arc atomic storage.
    #[inline]
    pub fn new(arc: A) -> Self {
        if D::ASYMMETRIC_BARRIERS {
            D::init_barriers();
        }
        Self {
            ptr: AtomicPtr::new(A::into_ptr(arc)),
            _arc: PhantomData,
            _domain: PhantomData,
            _write_policy: PhantomData,
        }
    }

    /// With asymmetric barriers, the heavy barrier ensures the store-load ordering of writes, both
    /// with loads and with the caller's SeqCst operations, so acquire-release is enough.
    const SWAP_ORDERING: Ordering = if D::ASYMMETRIC_BARRIERS {
        AcqRel
    } else {
        SeqCst
    };

    /// With asymmetric barriers, or without [`SEQ_CST_LOAD_IS_FREE`], the writer's scan starts
    /// with a store-load barrier (the heavy barrier or a SeqCst fence), so its slot loads can be
    /// relaxed.
    const SCAN_ORDERING: Ordering = if D::ASYMMETRIC_BARRIERS || !SEQ_CST_LOAD_IS_FREE {
        Relaxed
    } else {
        SeqCst
    };

    /// Loads a borrowed Arc.
    #[inline]
    pub fn load(&self) -> ArcPtrBorrow<A, D> {
        let ptr = self.ptr.load(if A::NULLABLE { SeqCst } else { Relaxed });
        self.load_impl::<false>(ptr)
    }

    #[inline(always)]
    fn load_impl<const INTO_OWNED: bool>(&self, ptr: *mut ()) -> ArcPtrBorrow<A, D> {
        if A::NULLABLE && ptr.is_null() {
            return ArcPtrBorrow::new(NULL, None);
        }
        debug_assert!(!ptr.is_null());
        debug_assert_eq!(ptr.tag(), 0, "`ArcPtr` pointers must be aligned to 4");
        let Some(node) = D::get_thread_local_node() else {
            return self.load_acquire_node::<INTO_OWNED>(ptr);
        };
        if INTO_OWNED || D::BORROW_SLOT_COUNT == 0 {
            return self.load_with_slot(ptr, node, node.extra_slot());
        }
        let slot = node.next_borrow_slot();
        // A free slot may have been released by a guard dropped in another thread. Acquire, so the
        // use of the released borrow happens before the (Release) store of the new one.
        // With asymmetric barriers, it also orders the reset of the releasing flag after its set.
        if !slot.load(Acquire).is_null() {
            return self.load_find_available_slot(ptr, node);
        }
        self.load_with_slot(ptr, node, slot)
    }

    #[cold]
    #[inline(never)]
    fn load_acquire_node<const INTO_OWNED: bool>(&self, ptr: *mut ()) -> ArcPtrBorrow<A, D> {
        D::get_or_acquire_thread_local_node();
        self.load_impl::<INTO_OWNED>(ptr)
    }

    #[inline(always)]
    fn load_with_slot(
        &self,
        ptr: *mut (),
        node: DomainNodeRef<D>,
        slot: SlotRef<D>,
    ) -> ArcPtrBorrow<A, D> {
        let ptr_checked = if D::ASYMMETRIC_BARRIERS {
            // Reset the releasing flag of the previous borrow, so the new one can be upgraded.
            slot.releasing().store(false, Relaxed);
            // Release, so a writer reading this borrow is synchronized with the release of the
            // previous one in `ArcPtrBorrow::drop` (a plain store is not part of its release
            // sequence), and observes the reset releasing flag.
            slot.store(ptr, Release);
            D::light_barrier();
            // Acquire is enough, as the light barrier ensures the store-load ordering, both with
            // writes (paired with the heavy barrier) and with the caller's SeqCst operations.
            self.ptr.load(Acquire)
        } else {
            slot.store(ptr, SeqCst);
            self.ptr.load(SeqCst)
        };
        if ptr != ptr_checked {
            return self.load_slow(ptr, node, slot);
        }
        let borrow = ArcPtrBorrow::<A, D>::new(ptr_checked, Some(slot));
        if node.is_extra_slot(slot) {
            return borrow.clone().into();
        }
        // SAFETY: `slot` is one of the node's borrow slots, but not the extra slot
        unsafe { node.set_next_borrow_slot_after(slot) };
        borrow
    }

    #[cold]
    #[inline(never)]
    fn load_find_available_slot(&self, ptr: *mut (), node: DomainNodeRef<D>) -> ArcPtrBorrow<A, D> {
        let slot = (node.borrow_slots().take(D::BORROW_SLOT_COUNT))
            // Acquire, see `load_impl`
            .find(|slot| slot.load(Acquire).is_null())
            .unwrap_or_else(|| node.extra_slot());
        self.load_with_slot(ptr, node, slot)
    }

    #[cold]
    #[inline(never)]
    fn load_slow(
        &self,
        ptr: *mut (),
        node: DomainNodeRef<D>,
        slot: SlotRef<D>,
    ) -> ArcPtrBorrow<A, D> {
        let self_ptr = ptr::from_ref(&self.ptr).cast_mut().cast();
        let loading_ptr = if W::CONCURRENT {
            // Release store, so the slot load following the acquire load in
            // `is_same_atomic_arc` cannot be stale.
            node.atomic_arc_slot().store(self_ptr, Release);
            let generation = node.next_loading_generation().get();
            #[cfg(not(hazarc_test_generation_wrap))]
            node.next_loading_generation()
                .set(generation.wrapping_add(TAG_MASK + 1));
            ptr::without_provenance_mut::<()>(generation).with_tag(LOADING)
        } else {
            self_ptr.with_tag(LOADING)
        };
        // An outdated borrow may have been upgraded, but it was never validated, so the writer
        // may come from another `AtomicArc` reusing the same allocation, possibly with another
        // pointer type. It must not be used, only released with the provided vtable.
        // The slot can only contain the outdated borrow, or its upgrade, so it can be swapped.
        let outdated_or_upgraded;
        let ptr_checked = if D::ASYMMETRIC_BARRIERS {
            // Release, so a writer reading the `LOADING` value observes `atomic_arc_slot`.
            let ordering = if W::CONCURRENT { Release } else { Relaxed };
            outdated_or_upgraded = slot.swap(loading_ptr, ordering);
            D::light_barrier();
            self.ptr.load(Acquire)
        } else {
            outdated_or_upgraded = slot.swap(loading_ptr, SeqCst);
            self.ptr.load(SeqCst)
        };
        // Relaxed success, as writers already observe the `LOADING` value, and the slot RMWs
        // extend the release sequence of the fast-path slot store. Acquire failure, so the
        // ownership transfer happens before the use of the owned pointer; its SeqCst semantic
        // comes from the writer's SeqCst load, which happens before the failure.
        let borrow = match slot.compare_exchange(loading_ptr, ptr_checked, Relaxed, Acquire) {
            // the slot is released if the pointer is null
            Ok(_) if A::NULLABLE && ptr_checked.is_null() => ArcPtrBorrow::<A, D>::new(NULL, None),
            Ok(_) => {
                let mut borrow = ArcPtrBorrow::<A, D>::new(ptr_checked, Some(slot));
                // The extra slot must be released before releasing the upgraded pointer,
                // as the latter may reenter `load`.
                if node.is_extra_slot(slot) {
                    borrow = borrow.clone().into();
                }
                borrow
            }
            Err(owned_ptr) => {
                debug_assert_eq!(owned_ptr.tag(), OWNED);
                // A writer thread never modifies an `OWNED` slot,
                // so it can be released with a store.
                slot.store(NULL, Release);
                ArcPtrBorrow::new(owned_ptr.without_tag(OWNED), None)
            }
        };
        // If the generation overflows, the node must be released to avoid ABA
        // (it can't be reacquired as long as it has ongoing swap's scans).
        // The release must happen after the last slot access, as it could be
        // reacquired by another concurrent load, but before the upgraded pointer
        // release, which could reenter `load`.
        #[cfg(any(not(target_pointer_width = "64"), hazarc_test_generation_wrap))]
        if W::CONCURRENT && node.next_loading_generation().get() == 0 {
            D::release_thread_local_node();
        }
        if outdated_or_upgraded != ptr {
            // Acquire, so the ownership transfer happens before the release
            // (already done by the SeqCst swap without asymmetric barriers).
            if D::ASYMMETRIC_BARRIERS {
                fence(Acquire);
            }
            let vtable = outdated_or_upgraded
                .without_tag(UPGRADED)
                .cast::<DropVTable>();
            // SAFETY: an upgraded slot contains a static vtable
            unsafe { ((*vtable).drop)(ptr) };
        }
        borrow
    }

    /// Loads an owned Arc.
    #[inline]
    pub fn load_owned(&self) -> A {
        let ptr = self.ptr.load(if A::NULLABLE { SeqCst } else { Relaxed });
        // SAFETY: `INTO_OWNED` loads return a borrow without slot
        unsafe { self.load_impl::<true>(ptr).into_owned_unchecked() }
    }

    #[cold]
    fn load_impl_cold(&self, ptr: *mut ()) -> ArcPtrBorrow<A, D> {
        self.load_impl::<false>(ptr)
    }

    /// Returns a reference to the cached Arc if it is up-to-date, or loads the latest Arc.
    #[inline]
    pub fn load_cached_or_reload<'a>(&self, cached: &'a A) -> CachedOrReloaded<'a, A, D> {
        let ptr = self.ptr.load(SeqCst);
        if ptr == A::as_ptr(cached) {
            CachedOrReloaded::Cached(cached)
        } else {
            CachedOrReloaded::Reloaded(self.load_impl_cold(ptr))
        }
    }

    #[cold]
    #[inline(never)]
    fn reload_cache(&self, ptr: *mut ()) -> A {
        // SAFETY: `INTO_OWNED` loads return a borrow without slot
        unsafe { self.load_impl::<true>(ptr).into_owned_unchecked() }
    }

    /// Returns a reference to the cached Arc, updating it when it is outdated.
    ///
    /// See [`Cache`](crate::Cache) for a convenient wrapper around this method.
    #[inline]
    pub fn load_cached<'a>(&self, cached: &'a mut A) -> &'a A {
        let ptr = self.ptr.load(SeqCst);
        if ptr != A::as_ptr(cached) {
            *cached = self.reload_cache(ptr);
        }
        cached
    }

    /// Stores the Arc and returns the previous one.
    pub fn swap(&self, arc: A) -> A {
        let old_ptr = self.ptr.swap(A::into_ptr(arc), Self::SWAP_ORDERING);
        self.swap_impl(old_ptr)
    }

    /// Scans all the domain's slots, providing an owned pointer to pending slow-path loads of
    /// this `AtomicArc`, and transferring the ownership of the old pointer to its borrows.
    fn swap_impl(&self, old_ptr: *mut ()) -> A {
        // With asymmetric barriers, the heavy barrier is executed after having registered an
        // active writer until the end of the scan. A guard drop with no active writer can then
        // reset its slot with a plain store, as the scan of all subsequent writes will observe
        // the slot's releasing flag, while the upgrades of the previous ones are visible.
        let mut _guard = None;
        if D::ASYMMETRIC_BARRIERS {
            _guard = Some(D::static_list().active_writer_guard());
            D::heavy_barrier();
        } else if !SEQ_CST_LOAD_IS_FREE {
            // The SeqCst fence allows relaxing the slot loads while preserving the Dekker
            // algorithm of slot reservation.
            fence(SeqCst);
        }
        // Leaked if the scan panics, as some borrows may not have been upgraded yet
        let old_arc = ManuallyDrop::new(unsafe { A::from_ptr(old_ptr) });
        let mut owned_cache: Option<ArcPtrBorrow<A, D>> = None;
        for node in D::static_list().nodes() {
            #[cfg(any(not(target_pointer_width = "64"), hazarc_test_generation_wrap))]
            let _guard = W::CONCURRENT.then(|| node.writer_guard());
            for slot in node.borrow_slots() {
                let mut slot_ptr = slot.load(Self::SCAN_ORDERING);
                if slot_ptr.is_null() {
                    continue;
                }
                if slot_ptr.tag() == LOADING && self.is_same_atomic_arc(node, slot, &mut slot_ptr) {
                    let owned = match owned_cache.as_mut() {
                        Some(cached) if W::CONCURRENT => cached.reload(self),
                        _ => owned_cache.get_or_insert_with(|| self.load()),
                    };
                    if let Err(ptr) = owned.transfer_ownership(|owned_ptr| {
                        // Release, so the ownership transfer happens before the release of the
                        // owned pointer by the loading thread.
                        slot.compare_exchange(slot_ptr, owned_ptr.with_tag(OWNED), Release, Relaxed)
                    }) {
                        // the slow-path load may have borrowed the old pointer in the meantime
                        slot_ptr = ptr;
                    }
                }
                // The slot may have been released after a `LOADING` value, so it can be null
                if slot_ptr == old_ptr && (!A::NULLABLE || !old_ptr.is_null()) {
                    // With asymmetric barriers, a releasing borrow must not be upgraded, as the
                    // slot may be reset with a plain store. Acquire fence, so the releasing flag
                    // is not older than the borrow (the flag is reset before storing it).
                    if D::ASYMMETRIC_BARRIERS {
                        fence(Acquire);
                        if slot.releasing().load(Relaxed) {
                            continue;
                        }
                    }
                    let _ = old_arc.transfer_ownership(|old_ptr| {
                        let vtable = ptr::from_ref(A::DROP_VTABLE).cast_mut().cast::<()>();
                        // Release, so the ownership transfer happens before the release of the old
                        // pointer by the borrow.
                        slot.compare_exchange(old_ptr, vtable.with_tag(UPGRADED), Release, Relaxed)
                    });
                }
            }
        }
        // Acquire fence, so borrow releases observed by relaxed slot loads (`SCAN_ORDERING`) or by
        // compare-exchange failures happen before the old arc is returned (and possibly dropped).
        fence(Acquire);
        ManuallyDrop::into_inner(old_arc)
    }

    fn is_same_atomic_arc(
        &self,
        node: DomainNodeRef<D>,
        slot: SlotRef<D>,
        slot_ptr: &mut *mut (),
    ) -> bool {
        let self_ptr = ptr::from_ref(&self.ptr).cast_mut().cast();
        if W::CONCURRENT {
            // With Relaxed `SCAN_ORDERING`, Acquire fence, so `atomic_arc_slot` stored before the
            // `LOADING` value (Release) is observed, as well as the loads of `self.ptr` preceding
            // it (for the monotonicity of the helping value).
            if Self::SCAN_ORDERING == Relaxed {
                fence(Acquire);
            }
            // Acquire load synchronizes with the release store in `load_slow`,
            // so the following slot load cannot be stale.
            // Without the synchronization, it would be possible to reload the
            // same generation while the atomic_arc_slot would come from a future
            // load of another `AtomicArc`, so the slot would be ignored.
            let same_atomic_arc = node.atomic_arc_slot().load(Acquire) == self_ptr;
            let prev_slot_ptr = *slot_ptr;
            // The slot is always reloaded, as a pending load may have completed by a borrow
            // of the swapped out arc.
            // Not part of the store-load ordering with loads, only Acquire to synchronize with
            // a borrow release, which the final Acquire fence does with relaxed `SCAN_ORDERING`.
            let ordering = if Self::SCAN_ORDERING == Relaxed {
                Relaxed
            } else {
                Acquire
            };
            *slot_ptr = slot.load(ordering);
            same_atomic_arc && *slot_ptr == prev_slot_ptr
        } else {
            *slot_ptr == self_ptr.with_tag(LOADING)
        }
    }

    /// Stores the Arc, dropping the previous one.
    pub fn store(&self, arc: A) {
        drop(self.swap(arc));
    }

    /// # Safety
    ///
    /// `self` must not be reused after.
    #[inline(always)]
    unsafe fn take_owned(&mut self) -> A {
        let ptr = self.ptr.load_mut();
        if A::NULLABLE && ptr.is_null() {
            return unsafe { A::from_ptr(NULL) };
        }
        // As there is no concurrent load, all borrows are visible. Asymmetric barrier normally
        // requires a heavy barrier, but it is not necessary here if there is no borrow.
        if D::ASYMMETRIC_BARRIERS {
            let mut slots = (D::static_list().nodes()).flat_map(|node| node.borrow_slots());
            if !slots.any(|slot| slot.load(Relaxed) == ptr) {
                // Acquire fence, so the borrow releases happen before the pointer is returned.
                fence(Acquire);
                return unsafe { A::from_ptr(ptr) };
            }
        }
        self.swap_impl(ptr)
    }

    /// Consumes the atomic storage and returns the stored Arc.
    #[inline]
    pub fn into_owned(self) -> A {
        // SAFETY: self is not reused after
        unsafe { ManuallyDrop::new(self).take_owned() }
    }
}

impl<A: ArcPtr, D: Domain> AtomicArcPtr<A, D, Concurrent> {
    /// Stores the new Arc if the current one matches the argument.
    ///
    /// Returns the previous Arc if store succeeds, or loads the current Arc otherwise.
    pub fn compare_exchange<C: ArcRef<A>>(
        &self,
        current: C,
        new: A,
    ) -> Result<A, ArcPtrBorrow<A, D>> {
        let new = ManuallyDrop::new(new);
        match (self.ptr).compare_exchange(
            C::as_ptr(current),
            A::as_ptr(&new),
            Self::SWAP_ORDERING,
            Acquire,
        ) {
            Ok(old_ptr) => Ok(self.swap_impl(old_ptr)),
            Err(old_ptr) => {
                drop(ManuallyDrop::into_inner(new));
                Err(self.load_impl::<false>(old_ptr))
            }
        }
    }

    /// Fetches the current Arc, applies a function on it and tries to store the result if
    /// the current Arc has not changed, retrying otherwise.
    ///
    /// Returns `Ok` with the previous Arc if the store succeeds, or `Err` with the current Arc
    /// if the function returns `None`.
    pub fn try_update<F: FnMut(&A) -> Option<R>, R: Into<A>>(
        &self,
        mut f: F,
    ) -> Result<A, ArcPtrBorrow<A, D>> {
        let mut current = self.load();
        while let Some(new) = f(&current) {
            match self.compare_exchange(&*current, new.into()) {
                Ok(old_arc) => return Ok(old_arc),
                Err(old_arc) => current = old_arc,
            }
        }
        Err(current)
    }

    /// Fetches the current Arc, applies a function on it and tries to store the result if
    /// the current Arc has not changed, retrying otherwise.
    ///
    /// Returns the previous Arc.
    pub fn update<F: FnMut(&A) -> R, R: Into<A>>(&self, mut f: F) -> A {
        self.try_update(|arc| Some(f(arc))).ok().unwrap()
    }

    #[doc(hidden)]
    #[deprecated(note = "use `try_update` instead")]
    pub fn fetch_update<F: FnMut(&A) -> Option<R>, R: Into<A>>(
        &self,
        f: F,
    ) -> Result<A, ArcPtrBorrow<A, D>> {
        self.try_update(f)
    }
}

impl<A: NonNullArcPtr, D: Domain, W: WritePolicy> AtomicArcPtr<Option<A>, D, W> {
    /// Constructs a new Arc atomic storage with `None`.
    #[inline]
    #[cfg_attr(loom, const_fn::const_fn(cfg(false)))]
    pub const fn none() -> Self {
        Self {
            ptr: AtomicPtr::new(NULL),
            _arc: PhantomData,
            _domain: PhantomData,
            _write_policy: PhantomData,
        }
    }

    /// Returns `true` if the stored Arc is `None`.
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

impl<A: NonNullArcPtr, D: Domain, W: WritePolicy> From<A> for AtomicArcPtr<Option<A>, D, W> {
    fn from(value: A) -> Self {
        Some(value).into()
    }
}

#[cfg(target_has_atomic = "ptr")]
impl<T, D: Domain, W: WritePolicy> From<T> for AtomicArcPtr<Arc<T>, D, W> {
    fn from(value: T) -> Self {
        Arc::new(value).into()
    }
}

#[cfg(target_has_atomic = "ptr")]
impl<T, D: Domain, W: WritePolicy> From<T> for AtomicArcPtr<Option<Arc<T>>, D, W> {
    fn from(value: T) -> Self {
        Some(Arc::new(value)).into()
    }
}

#[cfg(target_has_atomic = "ptr")]
impl<T, D: Domain, W: WritePolicy> From<Option<T>> for AtomicArcPtr<Option<Arc<T>>, D, W> {
    fn from(value: Option<T>) -> Self {
        value.map(Arc::new).into()
    }
}

/// A borrow of an `Arc`-like pointer, returned by [`AtomicArcPtr::load`].
///
/// In most cases, the borrowed Arc doesn't need to be cloned, hence the name.
#[must_use]
pub struct ArcPtrBorrow<A: ArcPtr, D: Domain> {
    arc: ManuallyDrop<A>,
    slot: Option<SlotRef<D>>,
}

impl<A: ArcPtr + fmt::Debug, D: Domain> fmt::Debug for ArcPtrBorrow<A, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArcPtrBorrow")
            .field("arc", &self.arc)
            .field("slot", &self.slot)
            .finish()
    }
}

impl<A: ArcPtr, D: Domain> ArcPtrBorrow<A, D> {
    #[inline(always)]
    pub(crate) fn new(ptr: *mut (), slot: Option<SlotRef<D>>) -> Self {
        let arc = ManuallyDrop::new(unsafe { A::from_ptr(ptr) });
        Self { arc, slot }
    }

    /// Convert the borrow into an owned Arc.
    ///
    /// The Arc may be cloned if it was not already the case.
    #[inline]
    pub fn into_owned(self) -> A {
        if self.slot.is_none() {
            // SAFETY: the borrow has no slot
            return unsafe { self.into_owned_unchecked() };
        }
        self.clone()
    }

    /// # Safety
    ///
    /// The borrow must have no slot.
    #[inline(always)]
    pub(crate) unsafe fn into_owned_unchecked(self) -> A {
        debug_assert!(self.slot.is_none());
        unsafe { ManuallyDrop::take(&mut ManuallyDrop::new(self).arc) }
    }

    /// Returns `true` if the borrow is outdated, i.e. the Arc currently stored in `atomic`
    /// is not the borrowed one.
    #[inline]
    pub fn is_outdated<W: WritePolicy>(&self, atomic: &AtomicArcPtr<A, D, W>) -> bool {
        atomic.ptr.load(SeqCst) != ArcPtr::as_ptr(&*self.arc)
    }

    /// Reload the borrow if it is outdated.
    #[inline]
    pub fn reload<W: WritePolicy>(&mut self, atomic: &AtomicArcPtr<A, D, W>) -> &mut Self {
        let ptr = atomic.ptr.load(SeqCst);
        if ptr != ArcPtr::as_ptr(&*self.arc) {
            *self = atomic.load_impl_cold(ptr);
        }
        self
    }
}

impl<A: NonNullArcPtr, D: Domain> ArcPtrBorrow<Option<A>, D> {
    /// Transpose a `ArcPtrBorrow<Option<A>, D>` into an `Option<ArcPtrBorrow<A, D>>`.
    ///
    /// This operation should be optimized out in release build.
    #[inline(always)]
    pub fn transpose(self) -> Option<ArcPtrBorrow<A, D>> {
        let this = ManuallyDrop::new(self);
        Some(ArcPtrBorrow::new(A::as_ptr(this.arc.as_ref()?), this.slot))
    }
}

impl<A: ArcPtr, D: Domain> Drop for ArcPtrBorrow<A, D> {
    #[inline]
    fn drop(&mut self) {
        let ptr = A::as_ptr(&self.arc);
        if let Some(slot) = self.slot {
            // With asymmetric barriers, the releasing flag of the slot is set before the light
            // barrier to make it visible from a further concurrent swap. A swap seeing this flag
            // will not try to upgrade it. If there is no swap ongoing after the barrier, the slot
            // can then be reset with a store, with the guarantee there is no concurrent upgrade.
            // Otherwise, the slot is reset with a swap to handle concurrent upgrade.
            let slot_ptr = if D::ASYMMETRIC_BARRIERS {
                // Release, so a writer skipping the releasing borrow is synchronized with its use.
                // The flag is reset by the next load using the slot.
                slot.releasing().store(true, Release);
                D::light_barrier();
                // Acquire is required to not miss a concurrent upgrade done by a completed swap.
                if D::static_list().active_writers().load(Acquire) == 0 {
                    let slot_ptr = slot.load(Relaxed);
                    // Release, so a load reusing the slot in another thread resets the releasing
                    // flag after it has been set.
                    slot.store(NULL, Release);
                    slot_ptr
                } else {
                    hint::cold_path();
                    slot.swap(NULL, Release)
                }
            } else {
                // Release, so the use of the Arc happens before its release, as slots are loaded
                // with SeqCst in `swap_impl`, or Relaxed followed by an acquire fence, and free
                // slots are loaded with Acquire before being reused.
                slot.swap(NULL, Release)
            };
            if slot_ptr == ptr {
                return;
            }
            debug_assert_eq!(slot_ptr.tag(), UPGRADED);
            // Acquire fence so the Arc clone in `swap_impl` happens before the following Arc drop.
            fence(Acquire);
        }
        #[cold]
        #[inline(never)]
        fn drop_arc<A>(_: A) {}
        drop_arc(unsafe { ManuallyDrop::take(&mut self.arc) });
    }
}

impl<A: ArcPtr, D: Domain> Deref for ArcPtrBorrow<A, D> {
    type Target = A;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.arc
    }
}

// NonNullPtr bound to avoid collision with `Option::as_ref`
impl<A: NonNullArcPtr, D: Domain> AsRef<A> for ArcPtrBorrow<A, D> {
    #[inline]
    fn as_ref(&self) -> &A {
        self
    }
}

impl<A: ArcPtr + fmt::Display, D: Domain> fmt::Display for ArcPtrBorrow<A, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<A: ArcPtr, D: Domain> From<A> for ArcPtrBorrow<A, D> {
    #[inline]
    fn from(value: A) -> Self {
        Self {
            arc: ManuallyDrop::new(value),
            slot: None,
        }
    }
}

impl<A: NonNullArcPtr, D: Domain> From<Option<ArcPtrBorrow<A, D>>> for ArcPtrBorrow<Option<A>, D> {
    #[inline]
    fn from(value: Option<ArcPtrBorrow<A, D>>) -> Self {
        match value.map(ManuallyDrop::new) {
            Some(a) => Self::new(A::as_ptr(&a.arc), a.slot),
            None => Self::new(NULL, None),
        }
    }
}

/// Result of [`AtomicArcPtr::load_cached_or_reload`].
pub enum CachedOrReloaded<'a, A: ArcPtr, D: Domain> {
    /// Cache Arc.
    Cached(&'a A),
    /// Reloaded Arc.
    Reloaded(ArcPtrBorrow<A, D>),
}

impl<A: ArcPtr + fmt::Debug, D: Domain> fmt::Debug for CachedOrReloaded<'_, A, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cached(arc) => f.debug_tuple("Cached").field(arc).finish(),
            Self::Reloaded(arc) => f.debug_tuple("Reloaded").field(arc).finish(),
        }
    }
}

impl<'a, A: NonNullArcPtr, D: Domain> CachedOrReloaded<'a, Option<A>, D> {
    /// Transpose a `CachedOrReloaded<Option<A>>` into an `Option<CacheReloaded<A>>`.
    pub fn transpose(self) -> Option<CachedOrReloaded<'a, A, D>> {
        match self {
            Self::Cached(arc) => arc.as_ref().map(CachedOrReloaded::Cached),
            Self::Reloaded(arc) => arc.transpose().map(CachedOrReloaded::Reloaded),
        }
    }
}

impl<A: ArcPtr, D: Domain> Deref for CachedOrReloaded<'_, A, D> {
    type Target = A;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Cached(arc) => arc,
            Self::Reloaded(arc) => arc,
        }
    }
}

/// A wrapper around `AtomicArcPtr<Option<A>>` with a more ergonomic API.
///
/// For example, [`AtomicOptionArcPtr::load`] returns an `Option<ArcPtrBorrow<A, D>>` instead of
/// `ArcPtrBorrow<Option<A>, D>`.
#[repr(transparent)]
pub struct AtomicOptionArcPtr<A: NonNullArcPtr, D: Domain, W: WritePolicy>(
    AtomicArcPtr<Option<A>, D, W>,
);

impl<A: NonNullArcPtr, D: Domain, W: WritePolicy> AtomicOptionArcPtr<A, D, W> {
    /// Constructs a new Arc atomic storage.
    #[inline]
    pub fn new(arc: Option<A>) -> Self {
        Self(AtomicArcPtr::new(arc))
    }

    /// Returns a reference to the inner `AtomicArcPtr`.
    #[inline]
    pub fn inner(&self) -> &AtomicArcPtr<Option<A>, D, W> {
        &self.0
    }

    /// Returns the inner `AtomicArcPtr`.
    #[inline]
    pub fn into_inner(self) -> AtomicArcPtr<Option<A>, D, W> {
        self.0
    }

    /// Constructs a new Arc atomic storage with `None`.
    #[inline]
    #[cfg_attr(loom, const_fn::const_fn(cfg(false)))]
    pub const fn none() -> Self {
        Self(AtomicArcPtr::none())
    }

    /// Returns `true` if the stored Arc is `None`.
    #[inline]
    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    /// Loads a borrowed Arc.
    #[inline]
    pub fn load(&self) -> Option<ArcPtrBorrow<A, D>> {
        self.0.load().transpose()
    }

    /// Loads an owned Arc.
    #[inline]
    pub fn load_owned(&self) -> Option<A> {
        self.0.load_owned()
    }

    /// Consumes the atomic storage and returns the stored Arc.
    #[inline]
    pub fn into_owned(self) -> Option<A> {
        self.0.into_owned()
    }

    /// Returns a reference to the cached Arc, updating it when it is outdated.
    ///
    /// See [`Cache`](crate::Cache) for a convenient wrapper around this method.
    #[inline]
    pub fn load_cached<'a>(&self, cached: &'a mut Option<A>) -> Option<&'a A> {
        self.0.load_cached(cached).as_ref()
    }

    /// Returns a reference to the cached Arc if it is up-to-date, or loads the latest Arc.
    #[inline]
    pub fn load_cached_or_reload<'a>(
        &self,
        arc: &'a Option<A>,
    ) -> Option<CachedOrReloaded<'a, A, D>> {
        self.0.load_cached_or_reload(arc).transpose()
    }

    /// Stores the Arc and returns the previous one.
    pub fn swap(&self, new: Option<A>) -> Option<A> {
        self.0.swap(new)
    }

    /// Stores the Arc, dropping the previous one.
    pub fn store(&self, new: Option<A>) {
        self.0.store(new);
    }
}

impl<A: NonNullArcPtr, D: Domain> AtomicOptionArcPtr<A, D, Concurrent> {
    /// Stores the new Arc if the current one matches the argument.
    ///
    /// Returns the previous Arc if store succeeds, or loads the current Arc otherwise.
    pub fn compare_exchange<C: ArcRef<Option<A>>>(
        &self,
        current: C,
        new: Option<A>,
    ) -> Result<Option<A>, Option<ArcPtrBorrow<A, D>>> {
        self.0
            .compare_exchange(current, new)
            .map_err(ArcPtrBorrow::transpose)
    }

    /// Fetches the current Arc, applies a function on it and tries to store the result if
    /// the current Arc has not changed, retrying otherwise.
    ///
    /// Returns `Ok` with the previous Arc if the store succeeds, or `Err` with the current Arc
    /// if the function returns `None`.
    pub fn try_update<F: FnMut(Option<&A>) -> Option<R>, R: Into<Option<A>>>(
        &self,
        mut f: F,
    ) -> Result<Option<A>, Option<ArcPtrBorrow<A, D>>> {
        self.0
            .try_update(|arc| f(arc.as_ref()))
            .map_err(ArcPtrBorrow::transpose)
    }

    /// Fetches the current Arc, applies a function on it and tries to store the result if
    /// the current Arc has not changed, retrying otherwise.
    ///
    /// Returns the previous Arc.
    pub fn update<F: FnMut(Option<&A>) -> R, R: Into<Option<A>>>(&self, mut f: F) -> Option<A> {
        self.0.update(|arc| f(arc.as_ref()))
    }

    #[doc(hidden)]
    #[deprecated(note = "use `try_update` instead")]
    pub fn fetch_update<F: FnMut(Option<&A>) -> Option<R>, R: Into<Option<A>>>(
        &self,
        f: F,
    ) -> Result<Option<A>, Option<ArcPtrBorrow<A, D>>> {
        self.try_update(f)
    }
}
impl<A: NonNullArcPtr, D: Domain, W: WritePolicy> Default for AtomicOptionArcPtr<A, D, W> {
    fn default() -> Self {
        Self::none()
    }
}

impl<A: NonNullArcPtr + fmt::Debug, D: Domain, W: WritePolicy> fmt::Debug
    for AtomicOptionArcPtr<A, D, W>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AtomicOptionArcPtr").field(&self.0).finish()
    }
}

impl<T, A: NonNullArcPtr, D: Domain, W: WritePolicy> From<T> for AtomicOptionArcPtr<A, D, W>
where
    AtomicArcPtr<Option<A>, D, W>: From<T>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl<'a, A: NonNullArcPtr, D: Domain, W: WritePolicy> From<&'a AtomicArcPtr<Option<A>, D, W>>
    for &'a AtomicOptionArcPtr<A, D, W>
{
    fn from(value: &'a AtomicArcPtr<Option<A>, D, W>) -> Self {
        unsafe { mem::transmute::<&'a AtomicArcPtr<Option<A>, D, W>, Self>(value) }
    }
}

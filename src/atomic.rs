//! Generic atomic storage for `Arc`-like pointers.

use alloc::sync::Arc;
use core::{
    fmt, hint,
    marker::PhantomData,
    mem,
    mem::ManuallyDrop,
    ops::Deref,
    sync::atomic::{
        fence, AtomicPtr,
        Ordering::{Acquire, Relaxed, Release, SeqCst},
    },
};

#[allow(unused_imports)]
use crate::msrv::{OptionExt, StrictProvenance};
use crate::{
    arc::{ArcPtr, ArcPtrExt, ArcRef, DropVTable, NonNullArcPtr},
    domain::{BorrowSlot, Domain, DomainNodeRef},
    msrv::ptr,
    write_policy::{Concurrent, WritePolicy},
    NULL,
};

// Slot tags, stored in the 2 LSBs (`ArcPtr` pointers are aligned to 4).
const TAG_MASK: usize = 0b11;
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
/// `LOADING` generations (i.e. ABA counter) for `Concurrent`, are stored above the tag bits.
const GENERATION_INCR: usize = TAG_MASK + 1;
/// Generation can overflow on 32-bit
const MAX_GENERATION: usize = !TAG_MASK;

trait TaggedPtr: Sized {
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
/// All atomic operations are [sequentially consistent](SeqCst) and
/// [pseudo wait-free](crate#wait-freedom).
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
        Self {
            ptr: AtomicPtr::new(A::into_ptr(arc)),
            _arc: PhantomData,
            _domain: PhantomData,
            _write_policy: PhantomData,
        }
    }

    /// Loads a borrowed Arc.
    #[inline]
    pub fn load(&self) -> ArcPtrBorrow<A> {
        self.load_impl(self.ptr.load(if A::NULLABLE { SeqCst } else { Relaxed }))
    }

    #[inline(always)]
    fn load_impl(&self, ptr: *mut ()) -> ArcPtrBorrow<A> {
        if A::NULLABLE && ptr.is_null() {
            return ArcPtrBorrow::new(NULL, None);
        }
        debug_assert!(!ptr.is_null());
        debug_assert_eq!(ptr.tag(), 0, "`ArcPtr` pointers must be aligned to 4");
        let node = D::get_or_acquire_thread_local_node();
        let slot_idx = match D::BORROW_SLOT_COUNT {
            // with no borrow slot, slot 0 is the extra slot, which is always available
            0 | 1 => 0,
            _ => node.next_borrow_slot_idx().get(),
        };
        let slot = unsafe { node.borrow_slots().get_unchecked(slot_idx) };
        let into_owned = D::BORROW_SLOT_COUNT == 0;
        if into_owned || slot.load(Relaxed).is_null() {
            self.load_with_slot(ptr, node, slot, slot_idx, into_owned)
        } else {
            self.load_find_available_slot(ptr, node)
        }
    }

    #[inline(always)]
    fn load_with_slot(
        &self,
        ptr: *mut (),
        node: DomainNodeRef<D>,
        slot: &'static BorrowSlot,
        slot_idx: usize,
        into_owned: bool,
    ) -> ArcPtrBorrow<A> {
        let ptr_checked = if D::ASYMMETRIC_BARRIERS {
            // Release, so a writer reading this borrow is synchronized with the release of the
            // previous one in `ArcPtrBorrow::drop` (a plain store is not part of its release
            // sequence).
            slot.store(ptr, Release);
            D::light_barrier();
            // SeqCst, as the checked pointer is the one returned, so it is the load's
            // linearization point (the initial load may be relaxed).
            self.ptr.load(SeqCst)
        } else {
            slot.store(ptr, SeqCst);
            self.ptr.load(SeqCst)
        };
        if ptr != ptr_checked {
            return self.load_slow(node, slot, ptr);
        }
        if into_owned {
            debug_assert_eq!(slot_idx, D::BORROW_SLOT_COUNT);
            return ArcPtrBorrow::<A>::new(ptr_checked, Some(slot))
                .clone()
                .into();
        }
        // The assertion is already known by compiler in `load_impl` with `get_unchecked`,
        // but it has to be repeated here to be taken into account for the modulo when borrow
        // slot count is not a multiple of 2
        if slot_idx >= D::BORROW_SLOT_COUNT {
            unsafe { hint::unreachable_unchecked() }; // MSRV 1.81
        }
        if D::BORROW_SLOT_COUNT > 1 {
            node.next_borrow_slot_idx()
                .set((slot_idx + 1) % D::BORROW_SLOT_COUNT);
        }
        ArcPtrBorrow::new(ptr_checked, Some(slot))
    }

    #[cold]
    #[inline(never)]
    fn load_find_available_slot(&self, ptr: *mut (), node: DomainNodeRef<D>) -> ArcPtrBorrow<A> {
        let (slot_idx, slot) = (node.borrow_slots().iter().enumerate())
            .find(|(_, slot)| slot.load(Relaxed).is_null())
            .expect("the last slot should always be available");
        self.load_with_slot(ptr, node, slot, slot_idx, slot_idx == D::BORROW_SLOT_COUNT)
    }

    #[cold]
    #[inline(never)]
    fn load_slow(
        &self,
        node: DomainNodeRef<D>,
        slot: &'static BorrowSlot,
        outdated_ptr: *mut (),
    ) -> ArcPtrBorrow<A> {
        let self_ptr = ptr::from_ref(&self.ptr).cast_mut().cast();
        let loading_ptr = if W::CONCURRENT {
            // Release store, so the slot load following the acquire load in
            // `is_same_atomic_arc` cannot be stale.
            node.atomic_arc_slot().store(self_ptr, Release);
            let generation = node.load_generation().get();
            node.load_generation()
                .set(generation.wrapping_add(GENERATION_INCR));
            if generation == MAX_GENERATION {
                D::release_thread_local_node();
            }
            ptr::without_provenance_mut::<()>(generation).with_tag(LOADING)
        } else {
            self_ptr.with_tag(LOADING)
        };
        // An outdated borrow may have been upgraded, but it was never validated, so the writer
        // may come from another `AtomicArc` reusing the same allocation, possibly with another
        // pointer type. It must not be used, only released with the provided vtable.
        // The slot can only contain the outdated borrow, or its upgrade, so it can be swapped.
        let outdated_or_upgraded;
        let ptr = if D::ASYMMETRIC_BARRIERS {
            // Release, so a writer reading the `LOADING` value observes `atomic_arc_slot`.
            let ordering = if W::CONCURRENT { Release } else { Relaxed };
            outdated_or_upgraded = slot.swap(loading_ptr, ordering);
            D::light_barrier();
            self.ptr.load(Acquire)
        } else {
            outdated_or_upgraded = slot.swap(loading_ptr, SeqCst);
            self.ptr.load(SeqCst)
        };
        // Failure ordering must be SeqCst for load to have a full SeqCst semantic
        let borrow = match slot.compare_exchange(loading_ptr, ptr, SeqCst, SeqCst) {
            // the slot is released if the pointer is null
            Ok(_) if A::NULLABLE && ptr.is_null() => ArcPtrBorrow::<A>::new(NULL, None),
            Ok(_) => {
                let mut borrow = ArcPtrBorrow::<A>::new(ptr, Some(slot));
                // The extra slot must be released before releasing the upgraded pointer,
                // as the latter may reenter `load`.
                let extra_slot = unsafe { node.borrow_slots().get_unchecked(D::BORROW_SLOT_COUNT) };
                if ptr::eq(slot, extra_slot) {
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
        if outdated_or_upgraded != outdated_ptr {
            // Acquire, so the ownership transfer happens before the release
            // (already done by the SeqCst swap without asymmetric barriers).
            if D::ASYMMETRIC_BARRIERS {
                fence(Acquire);
            }
            let vtable = outdated_or_upgraded
                .without_tag(UPGRADED)
                .cast::<DropVTable>();
            // SAFETY: an upgraded slot contains a static vtable
            unsafe { ((*vtable).drop)(outdated_ptr) };
        }
        borrow
    }

    /// Loads an owned Arc.
    #[inline]
    pub fn load_owned(&self) -> A {
        self.load().into_owned()
    }

    #[cold]
    fn load_impl_cold(&self, ptr: *mut ()) -> ArcPtrBorrow<A> {
        self.load_impl(ptr)
    }

    /// Returns a reference to the cached Arc if it is up-to-date, or loads the latest Arc.
    #[inline]
    pub fn load_cached_or_reload<'a>(&self, cached: &'a A) -> CachedOrReloaded<'a, A> {
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
        self.load_impl(ptr).into_owned()
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
        let old_ptr = self.ptr.swap(A::into_ptr(arc), SeqCst);
        if D::ASYMMETRIC_BARRIERS {
            D::heavy_barrier();
        }
        self.swap_impl(old_ptr)
    }

    /// Scans all the domain's slots, providing an owned pointer to pending slow-path loads of
    /// this `AtomicArc`, and transferring the ownership of the old pointer to its borrows.
    fn swap_impl(&self, old_ptr: *mut ()) -> A {
        // Leaked if the scan panics, as some borrows may not have been upgraded yet
        let old_arc = ManuallyDrop::new(unsafe { A::from_ptr(old_ptr) });
        let mut owned_cache: Option<ArcPtrBorrow<A>> = None;
        for node in D::static_list().nodes() {
            #[cfg(any(
                not(target_pointer_width = "64"),
                hazarc_force_active_writer_count_64bit
            ))]
            let _guard = W::CONCURRENT.then(|| node.writer_guard());
            for slot in node.borrow_slots() {
                let mut slot_ptr = slot.load(SeqCst);
                if slot_ptr.is_null() {
                    continue;
                }
                if slot_ptr.tag() == LOADING && self.is_same_atomic_arc(node, slot, &mut slot_ptr) {
                    let owned = match owned_cache.as_mut() {
                        Some(cached) if W::CONCURRENT => cached.reload(self),
                        _ => owned_cache.get_or_insert_with(|| self.load()),
                    };
                    if let Err(ptr) = owned.transfer_ownership(|owned_ptr| {
                        // Acquire failure, so the borrow release happens before the old pointer
                        // is returned (and possibly dropped)
                        slot.compare_exchange(slot_ptr, owned_ptr.with_tag(OWNED), SeqCst, Acquire)
                    }) {
                        // the slow-path load may have borrowed the old pointer in the meantime
                        slot_ptr = ptr;
                    }
                }
                // The slot may have been released after a `LOADING` value, so it can be null
                if slot_ptr == old_ptr && (!A::NULLABLE || !old_ptr.is_null()) {
                    let _ = old_arc.transfer_ownership(|old_ptr| {
                        let vtable = ptr::from_ref(A::DROP_VTABLE).cast_mut().cast::<()>();
                        // Acquire failure, so the borrow release happens before the old pointer
                        // is returned (and possibly dropped)
                        slot.compare_exchange(old_ptr, vtable.with_tag(UPGRADED), SeqCst, Acquire)
                    });
                }
            }
        }
        ManuallyDrop::into_inner(old_arc)
    }

    fn is_same_atomic_arc(
        &self,
        node: DomainNodeRef<D>,
        slot: &BorrowSlot,
        slot_ptr: &mut *mut (),
    ) -> bool {
        let self_ptr = ptr::from_ref(&self.ptr).cast_mut().cast();
        if W::CONCURRENT {
            // Acquire load synchronizes with the release store in `load_slow`,
            // so the following slot load cannot be stale.
            // Without the synchronization, it would be possible to reload the
            // same generation while the atomic_arc_slot would come from a future
            // load of another `AtomicArc`, so the slot would be ignored.
            let same_atomic_arc = node.atomic_arc_slot().load(Acquire) == self_ptr;
            let prev_slot_ptr = *slot_ptr;
            // The slot is always reloaded, as a pending load may have completed by a borrow
            // of the swapped out arc.
            *slot_ptr = slot.load(SeqCst);
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
        let ptr = *self.ptr.get_mut();
        if A::NULLABLE && ptr.is_null() {
            return unsafe { A::from_ptr(NULL) };
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
    pub fn compare_exchange<C: ArcRef<A>>(&self, current: C, new: A) -> Result<A, ArcPtrBorrow<A>> {
        let new = ManuallyDrop::new(new);
        match (self.ptr).compare_exchange(C::as_ptr(current), A::as_ptr(&new), SeqCst, Acquire) {
            Ok(old_ptr) => {
                if D::ASYMMETRIC_BARRIERS {
                    D::heavy_barrier();
                }
                Ok(self.swap_impl(old_ptr))
            }
            Err(old_ptr) => {
                drop(ManuallyDrop::into_inner(new));
                Err(self.load_impl(old_ptr))
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
    ) -> Result<A, ArcPtrBorrow<A>> {
        self.try_update(f)
    }
}

impl<A: NonNullArcPtr, D: Domain, W: WritePolicy> AtomicArcPtr<Option<A>, D, W> {
    /// Constructs a new Arc atomic storage with `None`.
    #[inline]
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

/// A borrow of an `Arc`-like pointer, returned by [`AtomicArcPtr::load`].
///
/// In most cases, the borrowed Arc doesn't need to be cloned, hence the name.
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

    /// Convert the borrow into an owned Arc.
    ///
    /// The Arc may be cloned if it was not already the case.
    #[inline]
    pub fn into_owned(self) -> A {
        if self.slot.is_none() {
            return unsafe { ManuallyDrop::take(&mut ManuallyDrop::new(self).arc) };
        }
        self.clone()
    }

    /// Reload the borrow if it is outdated.
    #[inline]
    pub fn reload<D: Domain, W: WritePolicy>(
        &mut self,
        atomic: &AtomicArcPtr<A, D, W>,
    ) -> &mut Self {
        let ptr = atomic.ptr.load(SeqCst);
        if ptr != ArcPtr::as_ptr(&*self.arc) {
            *self = atomic.load_impl_cold(ptr);
        }
        self
    }
}

impl<A: NonNullArcPtr> ArcPtrBorrow<Option<A>> {
    /// Transpose a `ArcPtrBorrow<Option<A>>` into an `Option<ArcPtrBorrow<A>>`.
    ///
    /// This operation should be optimized out in release build.
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
        // Reset the borrow slot to NULL with Release so the use of the Arc happens before its
        // release, as slots are loaded with SeqCst (Acquire) in `swap_impl`.
        if let Some(slot_ptr) = self.slot.map(|slot| slot.swap(NULL, Release)) {
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

impl<A: ArcPtr> Deref for ArcPtrBorrow<A> {
    type Target = A;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.arc
    }
}

// NonNullPtr bound to avoid collision with `Option::as_ref`
impl<A: NonNullArcPtr> AsRef<A> for ArcPtrBorrow<A> {
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

impl<A: NonNullArcPtr> From<Option<ArcPtrBorrow<A>>> for ArcPtrBorrow<Option<A>> {
    #[inline]
    fn from(value: Option<ArcPtrBorrow<A>>) -> Self {
        match value.map(ManuallyDrop::new) {
            Some(a) => Self::new(A::as_ptr(&a.arc), a.slot),
            None => Self::new(NULL, None),
        }
    }
}

/// Result of [`AtomicArcPtr::load_cached_or_reload`].
#[derive(Debug)]
pub enum CachedOrReloaded<'a, A: ArcPtr> {
    /// Cache Arc.
    Cached(&'a A),
    /// Reloaded Arc.
    Reloaded(ArcPtrBorrow<A>),
}

impl<'a, A: NonNullArcPtr> CachedOrReloaded<'a, Option<A>> {
    /// Transpose a `CachedOrReloaded<Option<A>>` into an `Option<CacheReloaded<A>>`.
    pub fn transpose(self) -> Option<CachedOrReloaded<'a, A>> {
        match self {
            Self::Cached(arc) => arc.as_ref().map(CachedOrReloaded::Cached),
            Self::Reloaded(arc) => arc.transpose().map(CachedOrReloaded::Reloaded),
        }
    }
}

impl<A: ArcPtr> Deref for CachedOrReloaded<'_, A> {
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
/// For example, [`AtomicOptionArcPtr::load`] returns an `Option<ArcPtrBorrow<A>>` instead of
/// `ArcPtrBorrow<Option<A>>`.
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
    pub fn load(&self) -> Option<ArcPtrBorrow<A>> {
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
    pub fn load_cached_or_reload<'a>(&self, arc: &'a Option<A>) -> Option<CachedOrReloaded<'a, A>> {
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
    ) -> Result<Option<A>, Option<ArcPtrBorrow<A>>> {
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
    ) -> Result<Option<A>, Option<ArcPtrBorrow<A>>> {
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
    ) -> Result<Option<A>, Option<ArcPtrBorrow<A>>> {
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

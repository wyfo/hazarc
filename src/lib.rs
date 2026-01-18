#![no_std]
extern crate alloc;

use alloc::{borrow::ToOwned, sync::Arc};
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

use borrow_list::StaticBorrowList;

use crate::borrow_list::{Borrow, BorrowNodeRef, release_borrow};

pub mod borrow_list;

const NULL: *mut () = ptr::null_mut();
const FALLBACK_FLAG: usize = 1;

pub struct AtomicArc<T, L> {
    ptr: AtomicPtr<()>,
    _ptr: PhantomData<T>,
    _list: PhantomData<L>,
}

unsafe impl<T, L> Send for AtomicArc<T, L> {}
unsafe impl<T, L> Sync for AtomicArc<T, L> {}

impl<T, L: StaticBorrowList> AtomicArc<T, L> {
    #[inline]
    pub fn new(arc: Arc<T>) -> Self {
        Self {
            ptr: AtomicPtr::new(Arc::into_raw(arc).cast_mut().cast()),
            _ptr: PhantomData,
            _list: PhantomData,
        }
    }

    #[inline]
    pub fn load(&self) -> ArcBorrow<T> {
        let ptr = self.ptr.load(Relaxed);
        let node = L::thread_local_node();
        let borrow_idx = node.next_borrow_idx.get();
        // `slots().get_unchecked` seems to ruin performance...
        let slot = unsafe { &*node.borrow_ptr().add(borrow_idx) };
        // let slot = unsafe { node.slots().get_unchecked(slot_idx) };
        if slot.load(Relaxed).is_null() {
            self.load_with_slot(ptr, node, slot, borrow_idx)
        } else {
            self.load_find_available_borrow(ptr, node)
        }
    }

    #[inline(always)]
    fn load_with_slot(
        &self,
        ptr: *mut (),
        node: BorrowNodeRef,
        borrow: &'static AtomicPtr<()>,
        borrow_idx: usize,
    ) -> ArcBorrow<T> {
        node.next_borrow_idx
            .set((borrow_idx + 1) & node.borrow_idx_mask);
        borrow.store(ptr.cast(), SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        if ptr != ptr_checked {
            return self.load_outdated(node, ptr, borrow);
        }
        ArcBorrow::new(ptr_checked, Some(borrow))
    }

    #[cold]
    #[inline(never)]
    fn load_outdated(
        &self,
        node: BorrowNodeRef,
        ptr: *mut (),
        borrow: &'static AtomicPtr<()>,
    ) -> ArcBorrow<T> {
        match borrow.compare_exchange(ptr.cast(), ptr::null_mut(), Release, Relaxed) {
            Ok(_) => self.load_fallback(node),
            Err(_) => ArcBorrow::new(ptr, None),
        }
    }

    #[cold]
    #[inline(never)]
    fn load_find_available_borrow(&self, ptr: *mut (), node: BorrowNodeRef) -> ArcBorrow<T> {
        match (node.borrows().iter().enumerate()).find(|(_, borrow)| borrow.load(Relaxed).is_null())
        {
            Some((slot_idx, slot)) => self.load_with_slot(ptr, node, slot, slot_idx),
            None => self.load_fallback(node),
        }
    }

    fn load_fallback(&self, node: BorrowNodeRef) -> ArcBorrow<T> {
        let init_ptr = ptr::without_provenance_mut(FALLBACK_FLAG);
        node.fallback.store(init_ptr, SeqCst);
        let ptr_checked = self.ptr.load(SeqCst);
        let confirm_ptr = ptr_checked.map_addr(|addr| addr | FALLBACK_FLAG).cast();
        let mut ptr_confirmed = ptr_checked;
        match (node.fallback).compare_exchange(init_ptr, confirm_ptr, Relaxed, Acquire) {
            Ok(_) => match (node.fallback).compare_exchange(confirm_ptr, NULL, Relaxed, Acquire) {
                Ok(_) => unsafe { Arc::increment_strong_count(ptr_checked) },
                Err(ptr) => debug_assert!(ptr.is_null()),
            },
            Err(ptr) => ptr_confirmed = ptr.cast(),
        }
        ArcBorrow::new(ptr_confirmed, None)
    }

    pub fn swap(&self, arc: Arc<T>) -> Arc<T> {
        let new_ptr = Arc::into_raw(arc).cast_mut();
        let old_ptr = self.ptr.swap(new_ptr.cast(), SeqCst);
        let old_arc = unsafe { Arc::from_raw(old_ptr.cast::<T>()) };
        // increment the refcount before in case some slots are reset
        let _guard = old_arc.clone();
        for node in L::static_list().nodes() {
            for borrow in node.borrows().iter() {
                if borrow.load(SeqCst) == old_ptr.cast()
                    && (borrow.compare_exchange(old_ptr.cast(), NULL, Release, Relaxed)).is_ok()
                {
                    unsafe { Arc::increment_strong_count(old_ptr) }
                }
            }
            let fallback_ptr = node.fallback.load(SeqCst);
            let fallback_xchg = match fallback_ptr.addr() {
                addr if addr == FALLBACK_FLAG => new_ptr.cast(),
                addr if addr == new_ptr.addr() | FALLBACK_FLAG => NULL,
                _ => continue,
            };
            // increment the refcount before in case fallback succeeds
            unsafe { Arc::increment_strong_count(new_ptr) };
            if (node.fallback)
                .compare_exchange(fallback_ptr, fallback_xchg, Release, Relaxed)
                .is_err()
            {
                unsafe { Arc::decrement_strong_count(new_ptr) };
            }
        }
        old_arc
    }

    pub fn store(&self, arc: Arc<T>) {
        drop(self.swap(arc));
    }
}

pub struct ArcBorrow<T> {
    borrow: Option<&'static Borrow>,
    arc: ManuallyDrop<Arc<T>>,
}

impl<T> ArcBorrow<T> {
    #[inline(always)]
    fn new(ptr: *mut (), borrow: Option<&'static Borrow>) -> Self {
        let arc = ManuallyDrop::new(unsafe { Arc::from_raw(ptr.cast()) });
        Self { borrow, arc }
    }

    #[inline]
    pub fn into_owned(self) -> Arc<T> {
        if self.borrow.is_none() {
            return unsafe { ManuallyDrop::take(&mut ManuallyDrop::new(self).arc) };
        }
        self.to_owned()
    }
}

impl<T> Drop for ArcBorrow<T> {
    #[inline]
    fn drop(&mut self) {
        if (self.borrow).is_none_or(|b| !release_borrow(b, Arc::as_ptr(&self.arc))) {
            #[cold]
            #[inline(never)]
            fn drop_arc<T>(_: Arc<T>) {}
            drop_arc(unsafe { ManuallyDrop::take(&mut self.arc) })
        }
    }
}

impl<T> Deref for ArcBorrow<T> {
    type Target = Arc<T>;
    fn deref(&self) -> &Self::Target {
        &self.arc
    }
}

#[macro_export]
macro_rules! borrow_list {
    ($vis:vis $name:ident($borrow_count:expr)) => {
        $vis struct $name;
        unsafe impl $crate::borrow_list::StaticBorrowList for $name {
            #[inline(always)]
            fn static_list() -> &'static $crate::borrow_list::BorrowList {
                static LIST: $crate::borrow_list::BorrowList = $crate::borrow_list::BorrowList::new();
                &LIST
            }
            #[inline(always)]
            fn thread_local_node() -> $crate::borrow_list::BorrowNodeRef {
                extern crate std;
                std::thread_local! {
                    static LOCAL: std::cell::Cell<std::option::Option<$crate::borrow_list::BorrowNodeRef>> = const { std::cell::Cell::new(None) };
                }
                #[cold]
                #[inline(never)]
                fn new_node() -> $crate::borrow_list::BorrowNodeRef {
                    struct NodeGuard;
                    impl Drop for NodeGuard {
                        fn drop(&mut self) {
                            if let Some(node) = LOCAL.take() {
                                unsafe { <$name as  $crate::borrow_list::StaticBorrowList>::static_list().remove_node(node) };
                            }
                        }
                    }
                    std::thread_local! {
                        static GUARD: NodeGuard = const { NodeGuard };
                    }
                    let node = <$name as  $crate::borrow_list::StaticBorrowList>::static_list().insert_node($borrow_count);
                    LOCAL.set(Some(node));
                    GUARD.with(|_| ());
                    node
                }
                LOCAL.get().unwrap_or_else(new_node)
            }
        }
    };
}

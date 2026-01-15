#![no_std]
extern crate alloc;

use alloc::{boxed::Box, sync::Arc};
use core::{
    cell::Cell,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::Deref,
    ptr,
    sync::atomic::{
        AtomicBool, AtomicPtr,
        Ordering::{Acquire, Relaxed, Release, SeqCst},
    },
};

use crossbeam_utils::CachePadded;

const SLOTS: usize = 8;
const FALLBACK_TRY_LOAD: usize = 1;
const FALLBACK_CONFIRM_LOAD: usize = 2;
const FALLBACK_PTR_MASK: usize = usize::MAX << 2;
const FALLBACK_STATE_MASK: usize = !FALLBACK_PTR_MASK;

#[derive(Debug, Default)]
pub struct Node {
    slots: CachePadded<[AtomicPtr<()>; SLOTS]>,
    next_slot: Cell<usize>,
    fallback_slot: AtomicPtr<()>,
    free: AtomicBool,
    next: AtomicPtr<Node>,
}

// SAFETY: `next_slot` access is synchronized with `free`
unsafe impl Send for Node {}
// SAFETY: `next_slot` access is synchronized with `free`
unsafe impl Sync for Node {}

pub struct List {
    head: AtomicPtr<Node>,
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

impl List {
    pub const fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn new_node(&self) -> &'static Node {
        let mut node_ptr = &self.head;
        while let Some(node) = unsafe { node_ptr.load(Acquire).as_ref() } {
            // Acquire load for `next_slot` synchronization
            if node.free.load(Relaxed)
                && node
                    .free
                    .compare_exchange(true, false, Acquire, Relaxed)
                    .is_ok()
            {
                return node;
            }
            node_ptr = &node.next;
        }
        let new_node = Box::leak(Default::default());
        while let Err(node_ref) = node_ptr
            .compare_exchange(ptr::null_mut(), ptr::from_mut(new_node), Release, Acquire)
            .map_err(|err| unsafe { err.as_ref().unwrap_unchecked() })
        {
            // No need to check free, because it's highly improbable
            // that a node is freed just after being added
            node_ptr = &node_ref.next;
        }
        new_node
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe trait Hazard {
    fn global() -> &'static List;
    fn local() -> &'static Node;
}

pub struct AtomicArc<T, H> {
    ptr: AtomicPtr<T>,
    _phantom: PhantomData<H>,
}

unsafe impl<T, H> Send for AtomicArc<T, H> {}
unsafe impl<T, H> Sync for AtomicArc<T, H> {}

impl<T, H: Hazard> AtomicArc<T, H> {
    pub fn new(arc: Arc<T>) -> Self {
        Self {
            ptr: AtomicPtr::new(Arc::into_raw(arc).cast_mut().cast()),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn load(&self) -> Guard<T> {
        let ptr = self.ptr.load(Relaxed);
        let node = H::local();
        let slot_idx = node.next_slot.get() % SLOTS;
        let slot = &node.slots[slot_idx];
        if slot.load(Relaxed).is_null() {
            self.load_with_slot(ptr, node, slot, slot_idx)
        } else {
            self.load_find_slot(ptr, node)
        }
    }

    #[inline(always)]
    pub fn load_local(&self, ptr: *mut T, node: &'static Node) -> Guard<T> {
        let slot_idx = node.next_slot.get() % SLOTS;
        let slot = &node.slots[slot_idx];
        if slot.load(Relaxed).is_null() {
            self.load_with_slot(ptr, node, slot, slot_idx)
        } else {
            self.load_find_slot(ptr, node)
        }
    }

    #[inline(always)]
    fn load_with_slot(
        &self,
        ptr: *mut T,
        node: &Node,
        slot: &'static AtomicPtr<()>,
        slot_idx: usize,
    ) -> Guard<T> {
        node.next_slot.set(slot_idx + 1);
        slot.store(ptr.cast(), SeqCst);
        let ptr_check = self.ptr.load(SeqCst);
        if ptr != ptr_check {
            return self.load_fallback(ptr, node);
        }
        Guard {
            slot: Some(slot),
            arc: ManuallyDrop::new(unsafe { Arc::from_raw(ptr_check) }),
        }
    }

    #[cold]
    #[inline(never)]
    fn load_find_slot(&self, ptr: *mut T, node: &'static Node) -> Guard<T> {
        match (node.slots.iter().enumerate()).find(|(_, slot)| slot.load(Relaxed).is_null()) {
            Some((slot_idx, slot)) => self.load_with_slot(ptr, node, slot, slot_idx),
            None => self.load_fallback(ptr, node),
        }
    }

    #[cold]
    #[inline(never)]
    fn load_fallback(&self, ptr: *mut T, node: &Node) -> Guard<T> {
        let ptr_try_load = ptr.map_addr(|addr| addr | FALLBACK_TRY_LOAD);
        node.fallback_slot.store(ptr_try_load.cast(), SeqCst);
        let ptr_check = self.ptr.load(SeqCst);
        let ptr_confirm_load = ptr_check.map_addr(|addr| addr | FALLBACK_CONFIRM_LOAD);
        let ptr_confirmed = match node.fallback_slot.compare_exchange(
            ptr_try_load.cast(),
            ptr_confirm_load.cast(),
            Relaxed,
            Acquire,
        ) {
            Ok(_) => {
                match node.fallback_slot.compare_exchange(
                    ptr_confirm_load.cast(),
                    ptr::null_mut(),
                    Relaxed,
                    Acquire,
                ) {
                    Ok(_) => unsafe { Arc::increment_strong_count(ptr_check) },
                    Err(ptr) => {
                        debug_assert!(ptr.is_null());
                    }
                };
                ptr_check
            }
            Err(ptr) => {
                debug_assert_eq!(ptr.addr() & FALLBACK_STATE_MASK, 0);
                ptr.cast()
            }
        };
        Guard {
            slot: None,
            arc: ManuallyDrop::new(unsafe { Arc::from_raw(ptr_confirmed) }),
        }
    }

    pub fn swap(&self, arc: Arc<T>) -> Arc<T> {
        let new_ptr = Arc::into_raw(arc).cast_mut();
        let old_ptr = self.ptr.swap(new_ptr.cast(), SeqCst);
        let mut node_ptr = &H::global().head;
        while let Some(node) = unsafe { node_ptr.load(Acquire).as_ref() } {
            for slot in node.slots.iter() {
                if slot.load(SeqCst) == old_ptr.cast()
                    && slot
                        .compare_exchange(old_ptr.cast(), ptr::null_mut(), Release, Relaxed)
                        .is_ok()
                {
                    unsafe { Arc::increment_strong_count(old_ptr) }
                }
            }
            let mut fallback_ptr = node.fallback_slot.load(SeqCst);
            while fallback_ptr.addr() & FALLBACK_STATE_MASK != 0 {
                if fallback_ptr.addr() & FALLBACK_STATE_MASK == FALLBACK_TRY_LOAD {
                    match node.fallback_slot.compare_exchange(
                        fallback_ptr,
                        new_ptr.cast(),
                        Release,
                        Relaxed,
                    ) {
                        Ok(_) => unsafe { Arc::increment_strong_count(new_ptr) },
                        Err(ptr) => fallback_ptr = ptr,
                    }
                } else if fallback_ptr.addr() & FALLBACK_STATE_MASK == FALLBACK_CONFIRM_LOAD {
                    if fallback_ptr.addr() & FALLBACK_PTR_MASK != new_ptr.addr() {
                        break;
                    }
                    match node.fallback_slot.compare_exchange(
                        fallback_ptr,
                        ptr::null_mut(),
                        Release,
                        Relaxed,
                    ) {
                        Ok(_) => unsafe { Arc::increment_strong_count(new_ptr) },
                        Err(ptr) => fallback_ptr = ptr,
                    }
                }
            }
            node_ptr = &node.next;
        }
        unsafe { Arc::from_raw(old_ptr) }
    }

    pub fn store(&self, arc: Arc<T>) {
        drop(self.swap(arc));
    }
}

pub struct Guard<T> {
    slot: Option<&'static AtomicPtr<()>>,
    arc: ManuallyDrop<Arc<T>>,
}

impl<T> Drop for Guard<T> {
    #[inline]
    fn drop(&mut self) {
        if let Some(slot) = self.slot
            && slot
                .compare_exchange(
                    Arc::as_ptr(&self.arc).cast_mut().cast(),
                    ptr::null_mut(),
                    Release,
                    Relaxed,
                )
                .is_ok()
        {
            return;
        }
        #[cold]
        #[inline(never)]
        fn drop_arc<T>(_: Arc<T>) {}
        drop_arc(unsafe { ManuallyDrop::take(&mut self.arc) })
    }
}

impl<T> Deref for Guard<T> {
    type Target = Arc<T>;
    fn deref(&self) -> &Self::Target {
        &self.arc
    }
}

#[cfg(feature = "std")]
pub struct Global;

#[cfg(feature = "std")]
unsafe impl Hazard for Global {
    #[inline(always)]
    fn global() -> &'static List {
        static LIST: List = List::new();
        &LIST
    }
    #[inline(always)]
    fn local() -> &'static Node {
        extern crate std;
        std::thread_local! {
            static LOCAL: Cell<Option<&'static Node>> = const { Cell::new(None) };
        }
        #[cold]
        #[inline(never)]
        fn new_node() -> &'static Node {
            struct NodeGuard;
            impl Drop for NodeGuard {
                fn drop(&mut self) {
                    if let Some(node) = LOCAL.take() {
                        // Release store for `next_slot` synchronization
                        node.free.store(true, Release);
                    }
                }
            }
            std::thread_local! {
                static GUARD: NodeGuard = const { NodeGuard };
            }
            let node = Global::global().new_node();
            LOCAL.set(Some(node));
            GUARD.with(|_| ());
            node
        }
        LOCAL.get().unwrap_or_else(new_node)
    }
}

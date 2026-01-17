#![no_std]
extern crate alloc;

use alloc::{
    alloc::{alloc_zeroed, handle_alloc_error},
    sync::Arc,
};
use core::{
    alloc::Layout,
    cell::Cell,
    iter,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::Deref,
    ptr,
    ptr::NonNull,
    slice,
    sync::atomic::{
        AtomicBool, AtomicPtr,
        Ordering::{Acquire, Relaxed, Release, SeqCst},
    },
};

use crossbeam_utils::CachePadded;

const NULL: *mut () = ptr::null_mut();

const FALLBACK_FLAG: usize = 1;

#[repr(C)]
pub struct Node {
    _align: CachePadded<()>,
    next: AtomicPtr<Node>,
    inserted: AtomicBool,
    fallback_slot: AtomicPtr<()>,
    slot_mask: usize,
    next_slot: Cell<usize>,
    __slots: [AtomicPtr<()>; 0],
}

// SAFETY: `next_slot` access is synchronized with `inserted`
unsafe impl Send for Node {}
// SAFETY: `next_slot` access is synchronized with `inserted`
unsafe impl Sync for Node {}

impl Node {
    fn allocate(slot_count: usize) -> NodeRef {
        let slot_count = slot_count.next_power_of_two();
        let (layout, _) = Layout::new::<Node>()
            .extend(Layout::array::<AtomicPtr<()>>(slot_count).unwrap())
            .unwrap();
        // SAFETY: layout has non-zero size
        let ptr = unsafe { alloc_zeroed(layout) }.cast::<Node>();
        let mut node = NodeRef(NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout)));
        unsafe { node.0.as_mut() }.slot_mask = slot_count - 1;
        *unsafe { node.0.as_mut() }.inserted.get_mut() = true;
        node
    }

    fn try_acquire(&self) -> bool {
        // Acquire load for `next_slot` synchronization
        !self.inserted.load(Relaxed) && !self.inserted.swap(true, Acquire)
    }

    unsafe fn release(&self) {
        // Release store for `next_slot` synchronization
        self.inserted.store(false, Release);
    }
}

#[derive(Clone, Copy)]
pub struct NodeRef(NonNull<Node>);
// SAFETY: `NodeRef` is equivalent to `&'static Node`
unsafe impl Send for NodeRef {}
// SAFETY: `NodeRef` is equivalent to `&'static Node`
unsafe impl Sync for NodeRef {}

impl Deref for NodeRef {
    type Target = Node;
    #[inline(always)]
    fn deref(&self) -> &Node {
        unsafe { self.0.as_ref() }
    }
}

impl NodeRef {
    unsafe fn new(ptr: *const Node) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(Self)
    }

    fn as_ptr(&self) -> *mut Node {
        self.0.as_ptr()
    }

    fn as_ref(&self) -> &'static Node {
        unsafe { self.0.as_ref() }
    }

    #[inline(always)]
    fn slots_ptr(self) -> *const AtomicPtr<()> {
        unsafe { &raw const (*self.0.as_ptr()).__slots as *const AtomicPtr<()> }
    }

    #[inline(always)]
    fn slots(self) -> &'static [AtomicPtr<()>] {
        unsafe { slice::from_raw_parts(self.slots_ptr(), self.slot_mask + 1) }
    }
}

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

    fn nodes(&self) -> impl Iterator<Item = NodeRef> {
        let mut node_ptr = &self.head;
        iter::from_fn(move || {
            let node = unsafe { NodeRef::new(node_ptr.load(Acquire))? };
            node_ptr = &node.as_ref().next;
            Some(node)
        })
    }

    pub fn insert_node(&self, slot_count: usize) -> NodeRef {
        let mut node_ptr = &self.head;
        // Cannot use `Self::nodes` because the final node pointer is needed for chaining
        while let Some(node) = unsafe { NodeRef::new(node_ptr.load(Acquire)) } {
            if node.try_acquire() {
                return node;
            }
            node_ptr = &node.as_ref().next;
        }
        let new_node = Node::allocate(slot_count);
        while let Err(node_ref) = node_ptr
            .compare_exchange(ptr::null_mut(), new_node.as_ptr(), Release, Relaxed)
            .map_err(|err| unsafe { err.as_ref().unwrap_unchecked() })
        {
            // No need to check free, because it's highly improbable
            // that a node is freed just after being added
            node_ptr = &node_ref.next;
        }
        new_node
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn remove_node(&self, node: NodeRef) {
        // SAFETY: same contract
        unsafe { node.release() };
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe trait HazardList {
    fn list() -> &'static List;
    fn local_node() -> NodeRef;
}

pub struct AtomicArc<T, L> {
    ptr: AtomicPtr<T>,
    _phantom: PhantomData<L>,
}

unsafe impl<T, L> Send for AtomicArc<T, L> {}
unsafe impl<T, L> Sync for AtomicArc<T, L> {}

impl<T, L: HazardList> AtomicArc<T, L> {
    #[inline]
    pub fn new(arc: Arc<T>) -> Self {
        Self {
            ptr: AtomicPtr::new(Arc::into_raw(arc).cast_mut().cast()),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn load(&self) -> Guard<T> {
        let ptr = self.ptr.load(Relaxed);
        let node = L::local_node();
        let slot_idx = node.next_slot.get();
        // `slots().get_unchecked` seems to ruin performance...
        let slot = unsafe { &*node.slots_ptr().add(slot_idx) };
        // let slot = unsafe { node.slots().get_unchecked(slot_idx) };
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
        node: NodeRef,
        slot: &'static AtomicPtr<()>,
        slot_idx: usize,
    ) -> Guard<T> {
        node.next_slot.set((slot_idx + 1) & node.slot_mask);
        slot.store(ptr.cast(), SeqCst);
        let ptr_check = self.ptr.load(SeqCst);
        if ptr != ptr_check {
            return self.load_fallback(node);
        }
        Guard {
            slot: Some(slot),
            arc: ManuallyDrop::new(unsafe { Arc::from_raw(ptr_check) }),
        }
    }

    #[cold]
    #[inline(never)]
    fn load_find_slot(&self, ptr: *mut T, node: NodeRef) -> Guard<T> {
        match (node.slots().iter().enumerate()).find(|(_, slot)| slot.load(Relaxed).is_null()) {
            Some((slot_idx, slot)) => self.load_with_slot(ptr, node, slot, slot_idx),
            None => self.load_fallback(node),
        }
    }

    #[cold]
    #[inline(never)]
    fn load_fallback(&self, node: NodeRef) -> Guard<T> {
        let init_ptr = ptr::without_provenance_mut(FALLBACK_FLAG);
        node.fallback_slot.store(init_ptr, SeqCst);
        let ptr_check = self.ptr.load(SeqCst);
        let confirm_ptr = ptr_check.map_addr(|addr| addr | FALLBACK_FLAG).cast();
        let mut ptr_confirmed = ptr_check;
        match (node.fallback_slot).compare_exchange(init_ptr, confirm_ptr, Relaxed, Acquire) {
            Ok(_) => {
                match (node.fallback_slot).compare_exchange(confirm_ptr, NULL, Relaxed, Acquire) {
                    Ok(_) => unsafe { Arc::increment_strong_count(ptr_check) },
                    Err(ptr) => debug_assert!(ptr.is_null()),
                }
            }
            Err(ptr) => ptr_confirmed = ptr.cast(),
        }
        Guard {
            slot: None,
            arc: ManuallyDrop::new(unsafe { Arc::from_raw(ptr_confirmed) }),
        }
    }

    pub fn swap(&self, arc: Arc<T>) -> Arc<T> {
        let new_ptr = Arc::into_raw(arc).cast_mut();
        let old_ptr = self.ptr.swap(new_ptr.cast(), SeqCst);
        let old_arc = unsafe { Arc::from_raw(old_ptr) };
        // increment the refcount before in case some slots are reset
        let _guard = old_arc.clone();
        for node in L::list().nodes() {
            for slot in node.slots().iter() {
                if slot.load(SeqCst) == old_ptr.cast()
                    && (slot.compare_exchange(old_ptr.cast(), NULL, Release, Relaxed)).is_ok()
                {
                    unsafe { Arc::increment_strong_count(old_ptr) }
                }
            }
            let fallback_ptr = node.fallback_slot.load(SeqCst);
            let fallback_xchg = match fallback_ptr.addr() {
                addr if addr == FALLBACK_FLAG => new_ptr.cast(),
                addr if addr == new_ptr.addr() | FALLBACK_FLAG => NULL,
                _ => continue,
            };
            // increment the refcount before in case fallback succeeds
            unsafe { Arc::increment_strong_count(new_ptr) };
            if (node.fallback_slot)
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

pub struct Guard<T> {
    slot: Option<&'static AtomicPtr<()>>,
    arc: ManuallyDrop<Arc<T>>,
}

impl<T> Drop for Guard<T> {
    #[inline]
    fn drop(&mut self) {
        if let Some(slot) = self.slot
            && let ptr = Arc::as_ptr(&self.arc).cast_mut().cast()
            && slot.compare_exchange(ptr, NULL, Release, Relaxed).is_ok()
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

#[macro_export]
macro_rules! hazard_list {
    ($vis:vis $name:ident($slot_count:expr)) => {
        $vis struct $name;
        unsafe impl $crate::HazardList for $name {
            #[inline(always)]
            fn list() -> &'static $crate::List {
                static LIST: $crate::List = $crate::List::new();
                &LIST
            }
            #[inline(always)]
            fn local_node() -> $crate::NodeRef {
                extern crate std;
                std::thread_local! {
                    static LOCAL: Cell<Option<$crate::NodeRef>> = const { Cell::new(None) };
                }
                #[cold]
                #[inline(never)]
                fn new_node() -> $crate::NodeRef {
                    struct NodeGuard;
                    impl Drop for NodeGuard {
                        fn drop(&mut self) {
                            if let Some(node) = LOCAL.take() {
                                unsafe { $name::list().remove_node(node) };
                            }
                        }
                    }
                    std::thread_local! {
                        static GUARD: NodeGuard = const { NodeGuard };
                    }
                    let node = $name::list().insert_node($slot_count);
                    LOCAL.set(Some(node));
                    GUARD.with(|_| ());
                    node
                }
                LOCAL.get().unwrap_or_else(new_node)
            }
        }
    };
}

#[cfg(feature = "std")]
hazard_list!(pub Global(8));

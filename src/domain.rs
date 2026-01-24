use alloc::{
    alloc::{alloc_zeroed, handle_alloc_error},
    vec::Vec,
};
use core::{
    alloc::Layout,
    cell::Cell,
    fmt, iter,
    ptr::NonNull,
    slice,
    sync::atomic::{
        AtomicBool, AtomicPtr,
        Ordering::{Acquire, Relaxed, Release, SeqCst},
    },
};

use crossbeam_utils::CachePadded;

use crate::NULL;

#[allow(clippy::missing_safety_doc)]
pub unsafe trait Domain: 'static + Send + Sync {
    fn static_list() -> &'static BorrowList;
    fn thread_local_node() -> BorrowNodeRef;
}

pub struct BorrowList {
    head: AtomicPtr<BorrowNode>,
}

impl Default for BorrowList {
    fn default() -> Self {
        Self::new()
    }
}

impl BorrowList {
    pub const fn new() -> Self {
        Self {
            head: AtomicPtr::new(NULL.cast()),
        }
    }

    pub(crate) fn nodes(&self) -> impl Iterator<Item = BorrowNodeRef> {
        let mut node_ptr = &self.head;
        iter::from_fn(move || {
            let node = unsafe { BorrowNodeRef::new(node_ptr.load(SeqCst))? };
            node_ptr = node.next();
            Some(node)
        })
    }

    pub fn insert_node(&self, slot_count: usize) -> BorrowNodeRef {
        let mut node_ptr = &self.head;
        // Cannot use `Self::nodes` because the final node pointer is needed for chaining
        while let Some(node) = unsafe { BorrowNodeRef::new(node_ptr.load(Acquire)) } {
            if node.try_acquire() {
                return node;
            }
            node_ptr = node.next();
        }
        let new_node = BorrowNodeRef::allocate(slot_count);
        while let Err(next) = node_ptr
            .compare_exchange(NULL.cast(), new_node.as_ptr(), SeqCst, Acquire)
            .map_err(|err| unsafe { &(*err).next })
        {
            // No need to check free, because it's highly improbable
            // that a node is freed just after being added
            node_ptr = next;
        }
        new_node
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn remove_node(&self, node: BorrowNodeRef) {
        // SAFETY: same contract
        unsafe { node.release() };
    }
}

impl fmt::Debug for BorrowList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowList")
            .field("nodes", &self.nodes().collect::<Vec<_>>())
            .finish()
    }
}

pub(crate) type BorrowSlot = AtomicPtr<()>;

#[repr(C)]
pub(crate) struct BorrowNode {
    _align: CachePadded<()>,
    next: AtomicPtr<BorrowNode>,
    in_use: AtomicBool,
    clone_slot: AtomicPtr<()>,
    next_borrow_slot_idx: Cell<usize>,
    borrow_slot_idx_mask: usize,
    borrow_slots: [BorrowSlot; 0],
}

// SAFETY: `next_slot` access is synchronized with `inserted`
unsafe impl Send for BorrowNode {}
// SAFETY: `next_slot` access is synchronized with `inserted`
unsafe impl Sync for BorrowNode {}

#[derive(Clone, Copy)]
pub struct BorrowNodeRef(NonNull<BorrowNode>);

// SAFETY: `NodeRef` is equivalent to `&'static Node`
unsafe impl Send for BorrowNodeRef {}
// SAFETY: `NodeRef` is equivalent to `&'static Node`
unsafe impl Sync for BorrowNodeRef {}

macro_rules! ref_field {
    ($field:ident: $ty:ty) => {
        #[inline(always)]
        pub(crate) fn $field(self) -> &'static $ty {
            unsafe { &(*self.as_ptr()).$field }
        }
    };
}

impl BorrowNodeRef {
    unsafe fn new(ptr: *const BorrowNode) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(Self)
    }

    fn allocate(borrows: usize) -> BorrowNodeRef {
        let borrows = borrows.next_power_of_two();
        let (layout, _) = Layout::new::<BorrowNode>()
            .extend(Layout::array::<AtomicPtr<()>>(borrows).unwrap())
            .unwrap();
        // SAFETY: layout has non-zero size
        let ptr = unsafe { alloc_zeroed(layout) }.cast::<BorrowNode>();
        let mut node =
            BorrowNodeRef(NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout)));
        unsafe { node.0.as_mut() }.borrow_slot_idx_mask = borrows - 1;
        *unsafe { node.0.as_mut() }.in_use.get_mut() = true;
        node
    }

    fn try_acquire(&self) -> bool {
        // Acquire load for `next_slot` synchronization
        !self.in_use().load(Relaxed) && !self.in_use().swap(true, Acquire)
    }

    unsafe fn release(&self) {
        // Release store for `next_slot` synchronization
        self.in_use().store(false, Release);
    }

    fn as_ptr(&self) -> *mut BorrowNode {
        self.0.as_ptr()
    }

    ref_field!(next: AtomicPtr<BorrowNode>);
    ref_field!(in_use: AtomicBool);
    ref_field!(clone_slot: AtomicPtr<()>);
    ref_field!(next_borrow_slot_idx: Cell<usize>);
    ref_field!(borrow_slot_idx_mask: usize);

    #[inline(always)]
    pub(crate) fn borrow_slots(self) -> &'static [BorrowSlot] {
        let len = self.borrow_slot_idx_mask() + 1;
        unsafe { slice::from_raw_parts(&raw const (*self.as_ptr()).borrow_slots as _, len) }
    }
}

impl fmt::Debug for BorrowNodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowNodeRef")
            .field("in_use", self.in_use())
            .field("borrow_slots", &self.borrow_slots())
            .field("clone_slot", self.clone_slot())
            .finish_non_exhaustive()
    }
}

#[macro_export]
macro_rules! domain {
    ($vis:vis $name:ident($borrow_slots:expr)) => {
        $vis struct $name;
        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let list = <$name as $crate::domain::Domain>::static_list();
                f.debug_tuple(::core::stringify!($name)).field(list).finish()
            }
        }
        unsafe impl $crate::domain::Domain for $name {
            #[inline(always)]
            fn static_list() -> &'static $crate::domain::BorrowList {
                static LIST: $crate::domain::BorrowList = $crate::domain::BorrowList::new();
                &LIST
            }
            #[inline(always)]
            fn thread_local_node() -> $crate::domain::BorrowNodeRef {
                extern crate std;
                std::thread_local! {
                    static LOCAL: std::cell::Cell<std::option::Option<$crate::domain::BorrowNodeRef>> = const { std::cell::Cell::new(None) };
                }
                #[cold]
                #[inline(never)]
                fn new_node() -> $crate::domain::BorrowNodeRef {
                    struct NodeGuard;
                    impl Drop for NodeGuard {
                        fn drop(&mut self) {
                            if let Some(node) = LOCAL.take() {
                                unsafe { <$name as  $crate::domain::Domain>::static_list().remove_node(node) };
                            }
                        }
                    }
                    std::thread_local! {
                        static GUARD: NodeGuard = const { NodeGuard };
                    }
                    let node = <$name as  $crate::domain::Domain>::static_list().insert_node($borrow_slots);
                    LOCAL.set(Some(node));
                    GUARD.with(|_| ());
                    node
                }
                LOCAL.get().unwrap_or_else(new_node)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::domain::Domain;

    domain!(TestList(1));
    #[test]
    fn node_reuse() {
        let thread = std::thread::spawn(|| {
            let node = TestList::thread_local_node();
            node.next_borrow_slot_idx().set(1);
            node
        });
        let node1 = thread.join().unwrap();
        let node2 = TestList::thread_local_node();
        assert_eq!(node1.as_ptr(), node2.as_ptr());
        assert_eq!(node2.next_borrow_slot_idx().get(), 1);
    }
}

use alloc::alloc::{alloc_zeroed, handle_alloc_error};
use core::{
    alloc::Layout,
    cell::Cell,
    iter,
    ptr::NonNull,
    slice,
    sync::atomic::{
        AtomicBool, AtomicPtr,
        Ordering::{Acquire, Relaxed, Release},
    },
};

use crossbeam_utils::CachePadded;

use crate::NULL;

#[allow(clippy::missing_safety_doc)]
pub unsafe trait StaticBorrowList {
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
            let node = unsafe { BorrowNodeRef::new(node_ptr.load(Acquire))? };
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
            .compare_exchange(NULL.cast(), new_node.as_ptr(), Release, Relaxed)
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

pub(crate) type Borrow = AtomicPtr<()>;

#[doc(hidden)]
#[repr(C)]
pub struct BorrowNode {
    _align: CachePadded<()>,
    next: AtomicPtr<BorrowNode>,
    inserted: AtomicBool,
    fallback: AtomicPtr<()>,
    next_borrow_idx: Cell<usize>,
    borrow_idx_mask: usize,
    borrows: [Borrow; 0],
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
        unsafe { node.0.as_mut() }.borrow_idx_mask = borrows - 1;
        *unsafe { node.0.as_mut() }.inserted.get_mut() = true;
        node
    }

    fn try_acquire(&self) -> bool {
        // Acquire load for `next_slot` synchronization
        !self.inserted().load(Relaxed) && !self.inserted().swap(true, Acquire)
    }

    unsafe fn release(&self) {
        // Release store for `next_slot` synchronization
        self.inserted().store(false, Release);
    }

    fn as_ptr(&self) -> *mut BorrowNode {
        self.0.as_ptr()
    }

    ref_field!(next: AtomicPtr<BorrowNode>);
    ref_field!(inserted: AtomicBool);
    ref_field!(fallback: AtomicPtr<()>);
    ref_field!(next_borrow_idx: Cell<usize>);
    ref_field!(borrow_idx_mask: usize);

    #[inline(always)]
    pub(crate) fn borrows(self) -> &'static [Borrow] {
        let len = self.borrow_idx_mask() + 1;
        unsafe { slice::from_raw_parts(&raw const (*self.as_ptr()).borrows as _, len) }
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

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::borrow_list::StaticBorrowList;

    borrow_list!(TestList(1));
    #[test]
    fn node_reuse() {
        let thread = std::thread::spawn(|| {
            let node = TestList::thread_local_node();
            node.next_borrow_idx().set(1);
            node
        });
        let node1 = thread.join().unwrap();
        let node2 = TestList::thread_local_node();
        assert_eq!(node1.as_ptr(), node2.as_ptr());
        assert_eq!(node2.next_borrow_idx().get(), 1);
    }
}

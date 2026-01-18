use alloc::alloc::{alloc_zeroed, handle_alloc_error};
use core::{
    alloc::Layout,
    cell::Cell,
    iter,
    ops::Deref,
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
            node_ptr = &node.as_ref().next;
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
            node_ptr = &node.as_ref().next;
        }
        let new_node = BorrowNode::allocate(slot_count);
        while let Err(node_ref) = node_ptr
            .compare_exchange(NULL.cast(), new_node.as_ptr(), Release, Relaxed)
            .map_err(|err| unsafe { err.as_ref().unwrap_unchecked() })
        {
            // No need to check free, because it's highly improbable
            // that a node is freed just after being added
            node_ptr = &node_ref.next;
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
    pub(crate) next: AtomicPtr<BorrowNode>,
    pub(crate) inserted: AtomicBool,
    pub(crate) fallback: AtomicPtr<()>,
    pub(crate) next_borrow_idx: Cell<usize>,
    pub(crate) borrow_idx_mask: usize,
    borrows: [Borrow; 0],
}

// SAFETY: `next_slot` access is synchronized with `inserted`
unsafe impl Send for BorrowNode {}

// SAFETY: `next_slot` access is synchronized with `inserted`
unsafe impl Sync for BorrowNode {}

impl BorrowNode {
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
        !self.inserted.load(Relaxed) && !self.inserted.swap(true, Acquire)
    }

    unsafe fn release(&self) {
        // Release store for `next_slot` synchronization
        self.inserted.store(false, Release);
    }
}

#[derive(Clone, Copy)]
pub struct BorrowNodeRef(NonNull<BorrowNode>);

// SAFETY: `NodeRef` is equivalent to `&'static Node`
unsafe impl Send for BorrowNodeRef {}

// SAFETY: `NodeRef` is equivalent to `&'static Node`
unsafe impl Sync for BorrowNodeRef {}

#[doc(hidden)]
impl Deref for BorrowNodeRef {
    type Target = BorrowNode;
    #[inline(always)]
    fn deref(&self) -> &BorrowNode {
        unsafe { self.0.as_ref() }
    }
}

impl BorrowNodeRef {
    unsafe fn new(ptr: *const BorrowNode) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(Self)
    }

    fn as_ptr(&self) -> *mut BorrowNode {
        self.0.as_ptr()
    }

    fn as_ref(&self) -> &'static BorrowNode {
        unsafe { self.0.as_ref() }
    }

    #[inline(always)]
    pub(crate) fn borrow_ptr(self) -> *const Borrow {
        unsafe { &raw const (*self.0.as_ptr()).borrows as *const Borrow }
    }

    #[inline(always)]
    pub(crate) fn borrows(self) -> &'static [Borrow] {
        unsafe { slice::from_raw_parts(self.borrow_ptr(), self.borrow_idx_mask + 1) }
    }
}

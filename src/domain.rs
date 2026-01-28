use alloc::{
    alloc::{alloc_zeroed, dealloc, handle_alloc_error},
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
        Ordering::{Acquire, Relaxed, SeqCst},
    },
};

use crossbeam_utils::CachePadded;

use crate::NULL;

macro_rules! node_field {
    ($node:ident.$field:ident) => {
        unsafe { &(*$node.as_ptr()).$field }
    };
}

#[allow(clippy::missing_safety_doc)]
pub unsafe trait Domain: 'static + Send + Sync {
    fn static_list() -> &'static BorrowList;
    fn thread_local_node() -> BorrowNodeRef;
    fn reset_thread_local_node();
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
            node_ptr = node_field!(node.next);
            Some(node)
        })
    }

    pub fn insert_node(&self, borrow_slot_count: usize) -> BorrowNodeRef {
        let mut node_ptr = &self.head;
        // Cannot use `Self::nodes` because the final node pointer is needed for chaining
        while let Some(node) = unsafe { BorrowNodeRef::new(node_ptr.load(Acquire)) } {
            if node.try_acquire() {
                return node;
            }
            node_ptr = node_field!(node.next);
        }
        let new_node = BorrowNodeRef::allocate(borrow_slot_count);
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

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn deallocate(&self) {
        let mut head = unsafe { BorrowNodeRef::new(self.head.swap(NULL.cast(), SeqCst)) };
        while let Some(node) = head {
            #[allow(unused_unsafe)]
            let next = unsafe { BorrowNodeRef::new(node_field!(node.next).load(Acquire)) };
            unsafe { node.deallocate() };
            head = next;
        }
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

impl BorrowNodeRef {
    unsafe fn new(ptr: *const BorrowNode) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(Self)
    }

    pub fn into_raw(self) -> NonNull<()> {
        self.0.cast()
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn from_raw(ptr: NonNull<()>) -> Self {
        Self(ptr.cast())
    }

    fn layout(borrow_slot_count: usize) -> Layout {
        assert!(borrow_slot_count.is_power_of_two());
        let (layout, _) = Layout::new::<BorrowNode>()
            .extend(Layout::array::<AtomicPtr<()>>(borrow_slot_count).unwrap())
            .unwrap();
        layout
    }

    fn allocate(borrow_slot_count: usize) -> BorrowNodeRef {
        let borrow_slot_count = borrow_slot_count.next_power_of_two();
        let layout = Self::layout(borrow_slot_count);
        // SAFETY: layout has non-zero size
        let ptr = unsafe { alloc_zeroed(layout) }.cast::<BorrowNode>();
        let mut node =
            BorrowNodeRef(NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout)));
        unsafe { node.0.as_mut() }.borrow_slot_idx_mask = borrow_slot_count - 1;
        *unsafe { node.0.as_mut() }.in_use.get_mut() = true;
        node
    }

    unsafe fn deallocate(self) {
        // load `self.in_use` to avoid data race
        let in_use = node_field!(self.in_use).load(SeqCst);
        debug_assert!(!in_use);
        debug_assert!((self.borrow_slots().iter()).all(|s| s.load(Relaxed).is_null()));
        debug_assert!(self.clone_slot().load(Relaxed).is_null());
        let layout = Self::layout(self.borrow_slots().len());
        unsafe { dealloc(self.as_ptr().cast(), layout) }
    }

    fn try_acquire(self) -> bool {
        // Acquire load for `next_slot` synchronization
        !node_field!(self.in_use).load(Relaxed) && !node_field!(self.in_use).swap(true, Acquire)
    }

    unsafe fn release(self) {
        // Release store for `next_slot` synchronization + SeqCst for `deallocate` synchronization
        node_field!(self.in_use).store(false, SeqCst);
    }

    fn as_ptr(self) -> *mut BorrowNode {
        self.0.as_ptr()
    }

    #[inline(always)]
    pub(crate) fn next_borrow_slot_idx(self) -> usize {
        node_field!(self.next_borrow_slot_idx).get()
    }
    #[inline(always)]
    pub(crate) fn set_next_borrow_slot_idx(self, slot_idx: usize) {
        node_field!(self.next_borrow_slot_idx)
            .set(slot_idx & node_field!(self.borrow_slot_idx_mask));
    }
    #[inline(always)]
    pub(crate) fn borrow_slots(self) -> &'static [BorrowSlot] {
        let len = node_field!(self.borrow_slot_idx_mask) + 1;
        unsafe { slice::from_raw_parts(&raw const (*self.as_ptr()).borrow_slots as _, len) }
    }

    pub(crate) fn clone_slot(self) -> &'static AtomicPtr<()> {
        node_field!(self.clone_slot)
    }
}

impl fmt::Debug for BorrowNodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowNodeRef")
            .field("in_use", node_field!(self.in_use))
            .field("borrow_slots", &self.borrow_slots())
            .field("clone_slot", node_field!(self.clone_slot))
            .finish_non_exhaustive()
    }
}

#[macro_export]
macro_rules! domain {
    ($vis:vis $name:ident($borrow_slot_count:expr)) => {
        #[derive(Debug)]
        $vis struct $name;
        impl $name {
            #[doc(hidden)]
            #[inline(always)]
            unsafe fn local_key() -> &'static ::std::thread::LocalKey<::std::cell::Cell<::std::option::Option<$crate::domain::BorrowNodeRef>>> {
                ::std::thread_local! {
                    static LOCAL: ::std::cell::Cell<::std::option::Option<$crate::domain::BorrowNodeRef>> = const { ::std::cell::Cell::new(None) };
                }
                &LOCAL
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
                #[cold]
                #[inline(never)]
                fn new_node() -> $crate::domain::BorrowNodeRef {
                    struct NodeGuard;
                    impl Drop for NodeGuard {
                        fn drop(&mut self) {
                            <$name as $crate::domain::Domain>::reset_thread_local_node();
                        }
                    }
                    ::std::thread_local! {
                        static GUARD: NodeGuard = const { NodeGuard };
                    }
                    let node = <$name as $crate::domain::Domain>::static_list().insert_node($borrow_slot_count);
                    unsafe { $name::local_key() }.set(Some(node));
                    GUARD.with(|_| ());
                    node
                }
                unsafe { $name::local_key() }.get().unwrap_or_else(new_node)
            }
            fn reset_thread_local_node() {
                if let Some(node) = unsafe { $name::local_key() }.take() {
                    unsafe { <$name as $crate::domain::Domain>::static_list().remove_node(node) };
                }
            }
        }
    };
}

#[cfg(feature = "pthread-domain")]
#[macro_export]
macro_rules! pthread_domain {
    ($vis:vis $name:ident) => {
        #[derive(Debug)]
        $vis struct $name;
        impl $name {
            #[doc(hidden)]
            #[inline(always)]
            pub fn key() -> *mut $crate::libc::pthread_key_t {
                static mut KEY: ::core::mem::MaybeUninit<$crate::libc::pthread_key_t> = core::mem::MaybeUninit::uninit();
                (&raw mut KEY).cast()
            }
            #[doc(hidden)]
            pub unsafe fn remove_node(ptr: *mut $crate::libc::c_void) {
                if let Some(ptr) = ::core::ptr::NonNull::new(ptr) {
                    let node = unsafe { $crate::domain::BorrowNodeRef::from_raw(ptr.cast()) };
                    unsafe { <$name as $crate::domain::Domain>::static_list().remove_node(node) };
                    unsafe { $crate::libc::pthread_setspecific(*$name::key(), ::core::ptr::null()) };
                }
            }
            #[inline]
            pub unsafe fn init_thread_local() {
                unsafe extern "C" fn make_key() {
                    unsafe extern "C" fn remove_node(ptr: *mut $crate::libc::c_void) {
                        unsafe { $name::remove_node(ptr) };
                    }
                    unsafe { $crate::libc::pthread_key_create($name::key(), Some(remove_node)) };
                }
                static mut KEY_ONCE: $crate::libc::pthread_once_t = $crate::libc::PTHREAD_ONCE_INIT;
                #[allow(clippy::missing_transmute_annotations)] // signature is different across platforms
                unsafe { $crate::libc::pthread_once(&raw mut KEY_ONCE, ::core::mem::transmute(make_key as unsafe extern "C" fn())) };
            }
        }
    };
    ($vis:vis $name:ident($borrow_slot_count:expr)) => {
        $crate::pthread_domain!($vis $name);
        unsafe impl $crate::domain::Domain for $name {
            $crate::pthread_domain_methods!($name($borrow_slot_count), unsafe { Self::init_thread_local() });
        }
    };
}

#[cfg(feature = "pthread-domain")]
#[macro_export]
macro_rules! pthread_domain_methods {
     ($name:ident($borrow_slot_count:expr)$(, $init:expr)?) => {
         #[inline(always)]
         fn static_list() -> &'static $crate::domain::BorrowList {
             static LIST: $crate::domain::BorrowList = $crate::domain::BorrowList::new();
             &LIST
         }
         #[inline(always)]
         fn thread_local_node() -> $crate::domain::BorrowNodeRef {
             #[cold]
             #[inline(never)]
             fn new_node() -> $crate::domain::BorrowNodeRef {
                 let node = <$name as $crate::domain::Domain>::static_list().insert_node ($borrow_slot_count);
                 unsafe { $crate::libc::pthread_setspecific(*$name::key(), node.into_raw().as_ptr().cast()) };
                 node
             }
             $($init;)?
             match unsafe { ::core::ptr::NonNull::new($crate::libc::pthread_getspecific(*Self::key())) } {
                 Some(ptr) => unsafe { $crate::domain::BorrowNodeRef::from_raw(ptr.cast()) },
                 None => new_node()
             }
         }
         fn reset_thread_local_node() {
             $($init;)?
             unsafe { $name::remove_node($crate::libc::pthread_getspecific(*Self::key())) };
         }
    };
}

#[cfg(test)]
mod tests {
    use crate::domain::{BorrowNodeRef, Domain};

    #[test]
    fn node_reuse() {
        #[cfg(feature = "pthread-domain")]
        pthread_domain!(TestDomain(2));
        #[cfg(not(feature = "pthread-domain"))]
        domain!(TestDomain(2));
        let thread = std::thread::spawn(|| {
            let node = TestDomain::thread_local_node();
            node.set_next_borrow_slot_idx(1);
            node
        });
        let node1 = thread.join().unwrap();
        let node2 = TestDomain::thread_local_node();
        assert_eq!(node1.as_ptr(), node2.as_ptr());
        assert_eq!(node2.next_borrow_slot_idx(), 1);
    }

    #[test]
    fn node_reuse2() {
        #[cfg(feature = "pthread-domain")]
        pthread_domain!(TestDomain(2));
        #[cfg(not(feature = "pthread-domain"))]
        domain!(TestDomain(2));
        std::thread::scope(|s| {
            let node1 = s.spawn(TestDomain::thread_local_node).join().unwrap();
            let thread2 = s.spawn(TestDomain::thread_local_node);
            let thread3 = s.spawn(TestDomain::thread_local_node);
            let node2 = thread2.join().unwrap();
            let node3 = thread3.join().unwrap();
            let ptr = |n| unsafe { core::mem::transmute::<BorrowNodeRef, *mut ()>(n) };
            assert!(ptr(node1) == ptr(node2) || ptr(node1) == ptr(node3));
        });
    }

    #[test]
    fn deallocation() {
        #[cfg(feature = "pthread-domain")]
        pthread_domain!(TestDomain(2));
        #[cfg(not(feature = "pthread-domain"))]
        domain!(TestDomain(2));
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|s| {
            s.spawn(|| {
                TestDomain::thread_local_node();
                barrier.wait();
                // It seems TLS can be destroyed after the thread has been joined...
                TestDomain::reset_thread_local_node();
            });
            s.spawn(|| {
                TestDomain::thread_local_node();
                barrier.wait();
                TestDomain::reset_thread_local_node();
            });
        });
        assert_eq!(TestDomain::static_list().nodes().count(), 2);
        unsafe { TestDomain::static_list().deallocate() };
        assert_eq!(TestDomain::static_list().nodes().count(), 0);
    }
}

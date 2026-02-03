//! Globally allocated helper for the `AtomicArc` algorithm.

use alloc::{
    alloc::{alloc_zeroed, dealloc, handle_alloc_error},
    vec::Vec,
};
use core::{
    alloc::Layout,
    cell::Cell,
    fmt, iter,
    marker::PhantomData,
    ptr::NonNull,
    slice,
    sync::atomic::{
        AtomicPtr, AtomicUsize,
        Ordering::{Acquire, Relaxed, SeqCst},
    },
};

use crossbeam_utils::CachePadded;

#[cfg(feature = "domain-gc")]
#[allow(unused_imports)]
use crate::msrv::StrictProvenance;
use crate::{msrv::ptr, NULL};

#[cfg(feature = "domain-gc")]
const GC_FLAG: usize = 1;
const IN_USE: usize = 1;

macro_rules! node_field {
    ($node:ident.$field:ident) => {
        unsafe { &(*$node.node.as_ptr()).$field }
    };
}

macro_rules! node_field_getter {
    ($field:ident: $ty:ty) => {
        #[inline(always)]
        pub(crate) fn $field(&self) -> &'static $ty {
            node_field!(self.$field)
        }
    };
}

/// TODO
///
/// # Safety
///
/// - [`static_list`](Self::static_list) must return a single static list.
/// - [`get_thread_local_node`](Self::get_thread_local_node) must return the node set with
///   [`set_thread_local_node`](Self::set_thread_local_node).
/// - [`get_or_acquire_thread_local_node`](Self::get_or_acquire_thread_local_node) and
///   [`release_thread_local_node`](Self::release_thread_local_node) should not be overwritten.
pub unsafe trait Domain: Sized + Send + Sync + 'static {
    /// Number of borrow slots of a thread-local node.
    const BORROW_SLOT_COUNT: usize;
    /// Returns the domain's static list.
    fn static_list() -> &'static DomainList<Self>;
    /// Returns the domain's thread-local node.
    fn get_thread_local_node() -> Option<DomainNodeRef<Self>>;
    /// Sets the domain's thread-local node.
    ///
    /// # Safety
    ///
    /// If not `None`, the node must have been acquired from the domain's static list,
    /// and must not have been released.
    unsafe fn set_thread_local_node(node: Option<DomainNodeRef<Self>>);
    /// Gets the domain's thread-local node, or acquires one from the static list and cache it.
    fn get_or_acquire_thread_local_node() -> DomainNodeRef<Self> {
        #[cold]
        #[inline(never)]
        fn acquire_node<D: Domain>() -> DomainNodeRef<D> {
            let node = D::static_list().acquire_node();
            unsafe { D::set_thread_local_node(Some(node)) };
            node
        }
        Self::get_thread_local_node().unwrap_or_else(acquire_node::<Self>)
    }
    /// Releases the domain's thread-local node, if there is one stored.
    fn release_thread_local_node() {
        if let Some(node) = Self::get_thread_local_node() {
            unsafe { Self::set_thread_local_node(None) };
            unsafe { Self::static_list().release_node(node) };
        }
    }
}

/// The domain's list.
///
/// See [`Domain`] documentation.
pub struct DomainList<D> {
    head: AtomicPtr<DomainNode>,
    #[cfg(feature = "domain-gc")]
    active_nodes_and_writers: AtomicUsize,
    _domain: PhantomData<D>,
}

impl<D: Domain> Default for DomainList<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: Domain> DomainList<D> {
    /// Constructs a new, empty `DomainList`.
    pub const fn new() -> Self {
        Self {
            head: AtomicPtr::new(NULL.cast()),
            #[cfg(feature = "domain-gc")]
            active_nodes_and_writers: AtomicUsize::new(0),
            _domain: PhantomData,
        }
    }

    pub(crate) fn nodes(&self) -> impl Iterator<Item = DomainNodeRef<D>> + '_ {
        let guard = ListAccessGuard::new(self);
        let mut node = None::<DomainNodeRef<D>>;
        iter::from_fn(move || {
            node = node.map_or_else(|| guard.head(), |n| n.next());
            node
        })
    }

    pub(crate) fn nodes_or_allocate(&self) -> impl Iterator<Item = DomainNodeRef<D>> + '_ {
        struct AllocatedNode<D: Domain>(Option<DomainNodeRef<D>>);
        impl<D: Domain> Drop for AllocatedNode<D> {
            fn drop(&mut self) {
                if let Some(node) = self.0.take() {
                    unsafe { node.deallocate() }
                }
            }
        }
        let guard = ListAccessGuard::new(self);
        let mut node = None::<DomainNodeRef<D>>;
        let mut allocated_node = AllocatedNode(None);
        iter::from_fn(move || {
            let node_ptr = node.map_or(&self.head, |n| node_field!(n.next));
            node = node.map_or_else(|| guard.head(), |n| n.next());
            if node.is_none() {
                let new_node =
                    (allocated_node.0).get_or_insert_with(|| DomainNodeRef::<D>::allocate());
                match node_ptr.compare_exchange(NULL.cast(), new_node.as_ptr(), SeqCst, Acquire) {
                    Ok(_) => node = allocated_node.0.take(),
                    Err(n) => node = unsafe { DomainNodeRef::new(n) },
                }
            }
            debug_assert!(node.is_some());
            node
        })
    }

    /// Reserve `node_count` nodes in the list.
    ///
    /// This method doesn't take in account if the nodes are acquired or not, it just makes sure
    /// there are at least `node_count` nodes allocated.
    pub fn reserve(&'static self, node_count: usize) {
        for _ in self.nodes_or_allocate().take(node_count) {}
    }

    /// Acquire a node from the list.
    ///
    /// If no node has been reserved or released, a new node is allocated.
    pub fn acquire_node(&self) -> DomainNodeRef<D> {
        for node in self.nodes_or_allocate() {
            if node.try_acquire() {
                core::mem::forget(ListAccessGuard::new(self));
                return node;
            }
        }
        unreachable!()
    }

    /// Release the node, keeping it in the list for another thread to acquire it.
    ///
    /// # Safety
    ///
    /// The node must have been acquired from the list and not have been released yet.
    pub unsafe fn release_node(&self, node: DomainNodeRef<D>) {
        // SAFETY: same contract
        unsafe { node.release() };
        drop(ListAccessGuard(self));
    }

    /// Deallocates the list.
    ///
    /// # Safety
    ///
    /// All list's nodes must have been released.
    pub unsafe fn deallocate(&self) {
        #[cfg(feature = "domain-gc")]
        debug_assert_eq!(self.active_nodes_and_writers.load(SeqCst), 0);
        let mut head = unsafe { DomainNodeRef::<D>::new(self.head.swap(NULL.cast(), SeqCst)) };
        while let Some(node) = head {
            #[allow(unused_unsafe)]
            let next = unsafe { DomainNodeRef::new(node_field!(node.next).load(Acquire)) };
            unsafe { node.deallocate() };
            head = next;
        }
    }
}

impl<D: Domain> fmt::Debug for DomainList<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DomainList")
            .field("domain", &core::any::type_name::<D>())
            .field("nodes", &self.nodes().collect::<Vec<_>>())
            .finish()
    }
}

struct ListAccessGuard<'a, D: Domain>(&'a DomainList<D>);

impl<'a, D: Domain> ListAccessGuard<'a, D> {
    fn new(list: &'a DomainList<D>) -> Self {
        #[cfg(feature = "domain-gc")]
        list.active_nodes_and_writers
            .fetch_add(1 << GC_FLAG, SeqCst);
        Self(list)
    }

    fn head(&self) -> Option<DomainNodeRef<D>> {
        #[cfg_attr(not(feature = "domain-gc"), allow(unused_mut))]
        let mut head = self.0.head.load(SeqCst);
        #[cfg(feature = "domain-gc")]
        if head.addr() & GC_FLAG != 0 {
            let new_head = head.map_addr(|addr| addr & !GC_FLAG);
            match self.0.head.compare_exchange(head, new_head, SeqCst, SeqCst) {
                Ok(_) => head = new_head,
                Err(h) => head = h,
            }
        }
        unsafe { DomainNodeRef::new(head) }
    }
}

impl<D: Domain> Drop for ListAccessGuard<'_, D> {
    fn drop(&mut self) {
        #[cfg(feature = "domain-gc")]
        if (self.0.active_nodes_and_writers).fetch_sub(1 << GC_FLAG, SeqCst) == 1 << GC_FLAG {
            if (self.0.active_nodes_and_writers)
                .compare_exchange(0, GC_FLAG, SeqCst, SeqCst)
                .is_err()
            {
                return;
            }
            let head = self.0.head.fetch_or(GC_FLAG, SeqCst);
            let flagged_head = head.map_addr(|addr| addr | GC_FLAG);
            if !head.is_null()
                && self.0.active_nodes_and_writers.load(SeqCst) == GC_FLAG
                && (self.0.head)
                    .compare_exchange(flagged_head, NULL.cast(), SeqCst, Relaxed)
                    .is_ok()
            {
                let mut head = unsafe { DomainNodeRef::<D>::new(head) };
                while let Some(node) = head {
                    head = node.next();
                    unsafe { node.deallocate() };
                }
            }
            self.0.active_nodes_and_writers.fetch_and(!GC_FLAG, SeqCst);
        }
    }
}

pub(crate) type BorrowSlot = AtomicPtr<()>;

#[repr(C)]
pub(crate) struct DomainNode {
    _align: CachePadded<()>,
    next: AtomicPtr<DomainNode>,
    in_use: AtomicUsize,
    clone_slot: AtomicPtr<()>,
    atomic_arc_slot: AtomicPtr<()>,
    clone_generation: Cell<usize>,
    next_borrow_slot_idx: Cell<usize>,
    borrow_slots: [BorrowSlot; 0],
}

/// A static reference to a domain's node.
///
/// It is guaranteed that `size_of::<Option<DomainNodeRef<D>>() == size_of::<usize>()`.
///
/// See [`Domain`] documentation.
pub struct DomainNodeRef<D> {
    node: NonNull<DomainNode>,
    _domain: PhantomData<D>,
}

impl<D> Clone for DomainNodeRef<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for DomainNodeRef<D> {}

impl<D: Domain> DomainNodeRef<D> {
    #[allow(unstable_name_collisions)]
    unsafe fn new(ptr: *const DomainNode) -> Option<Self> {
        #[cfg(feature = "domain-gc")]
        let ptr = ptr.map_addr(|addr| addr & !GC_FLAG);
        Some(Self {
            node: NonNull::new(ptr.cast_mut())?,
            _domain: PhantomData,
        })
    }

    pub fn into_raw(self) -> NonNull<()> {
        self.node.cast()
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn from_raw(ptr: NonNull<()>) -> Self {
        Self {
            node: ptr.cast(),
            _domain: PhantomData,
        }
    }

    fn layout() -> Layout {
        let (layout, _) = Layout::new::<DomainNode>()
            .extend(Layout::array::<AtomicPtr<()>>(D::BORROW_SLOT_COUNT).unwrap())
            .unwrap();
        layout
    }

    fn allocate() -> DomainNodeRef<D> {
        let layout = Self::layout();
        // SAFETY: layout has non-zero size
        let ptr = unsafe { alloc_zeroed(layout) }.cast();
        DomainNodeRef {
            node: NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout)),
            _domain: PhantomData,
        }
    }

    unsafe fn deallocate(self) {
        // load `self.in_use` to avoid data race
        let in_use = node_field!(self.in_use).load(SeqCst);
        debug_assert_eq!(in_use, 0);
        debug_assert!((self.borrow_slots().iter()).all(|s| s.load(Relaxed).is_null()));
        debug_assert!(self.clone_slot().load(Relaxed).is_null());
        let layout = Self::layout();
        unsafe { dealloc(self.as_ptr().cast(), layout) }
    }

    fn try_acquire(self) -> bool {
        node_field!(self.in_use).load(Relaxed) == 0
            && node_field!(self.in_use)
                // Acquire load for `next_slot` synchronization
                .compare_exchange(0, IN_USE, SeqCst, Relaxed)
                .is_ok()
    }

    unsafe fn release(self) {
        // Release store for `next_slot` synchronization + SeqCst for `deallocate` synchronization
        node_field!(self.in_use).fetch_and(!IN_USE, SeqCst);
    }

    #[cfg(any(
        not(target_pointer_width = "64"),
        hazarc_force_active_writer_count_64bit
    ))]
    pub(crate) fn writer_guard(self) -> WriterGuard<D> {
        let in_use = node_field!(self.in_use).fetch_add(1 << IN_USE, SeqCst);
        if in_use >= usize::MAX / 2 {
            struct PanicInDrop;
            impl Drop for PanicInDrop {
                fn drop(&mut self) {
                    panic!("panic in drop");
                }
            }
            let _guard = PanicInDrop;
            panic!("too many concurrent writers; abort");
        }
        WriterGuard(self)
    }

    fn as_ptr(self) -> *mut DomainNode {
        self.node.as_ptr()
    }

    fn next(self) -> Option<DomainNodeRef<D>> {
        #[allow(unused_unsafe)]
        unsafe {
            Self::new(node_field!(self.next).load(SeqCst))
        }
    }

    #[inline(always)]
    pub(crate) fn borrow_slots(self) -> &'static [BorrowSlot] {
        unsafe {
            let slots = ptr::addr_of!((*self.as_ptr()).borrow_slots);
            slice::from_raw_parts(slots as _, D::BORROW_SLOT_COUNT)
        }
    }

    node_field_getter!(next_borrow_slot_idx: Cell<usize>);
    node_field_getter!(clone_slot: AtomicPtr<()>);
    node_field_getter!(atomic_arc_slot: AtomicPtr<()>);
    node_field_getter!(clone_generation: Cell<usize>);
}

impl<D: Domain> fmt::Debug for DomainNodeRef<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let in_use = node_field!(self.in_use).load(Relaxed);
        f.debug_struct("DomainNodeRef")
            .field("domain", &core::any::type_name::<D>())
            .field("in_use", &(in_use & IN_USE != 0))
            .field("active_writers", &(in_use >> IN_USE))
            .field("borrow_slots", &self.borrow_slots())
            .field("clone_slot", node_field!(self.clone_slot))
            .finish_non_exhaustive()
    }
}

#[cfg(any(
    not(target_pointer_width = "64"),
    hazarc_force_active_writer_count_64bit
))]
pub(crate) struct WriterGuard<D>(DomainNodeRef<D>);

#[cfg(any(
    not(target_pointer_width = "64"),
    hazarc_force_active_writer_count_64bit
))]
impl<D> Drop for WriterGuard<D> {
    fn drop(&mut self) {
        let node = self.0;
        node_field!(node.in_use).fetch_sub(1 << IN_USE, SeqCst);
    }
}

/// Declare a domain using standard thread-local storage.
///
/// # Examples
///
/// ```rust
/// hazarc::domain!(pub(crate) MyDomain(2)); // 2 borrow slots
/// ```
#[macro_export]
macro_rules! domain {
    ($(#[$attrs:meta])* $vis:vis $name:ident($borrow_slot_count:expr)) => {
        $(#[$attrs])*
        #[derive(Debug)]
        $vis struct $name;
        impl $name {
            #[doc(hidden)]
            #[inline(always)]
            fn local_key() -> &'static ::std::thread::LocalKey<::std::cell::Cell<::std::option::Option<$crate::domain::DomainNodeRef<Self>>>> {
                ::std::thread_local! {
                    static LOCAL: ::std::cell::Cell<::std::option::Option<$crate::domain::DomainNodeRef<$name>>> = const { ::std::cell::Cell::new(None) };
                }
                &LOCAL
            }
        }
        unsafe impl $crate::domain::Domain for $name {
            const BORROW_SLOT_COUNT: usize = $borrow_slot_count;
            #[inline(always)]
            fn static_list() -> &'static $crate::domain::DomainList<Self> {
                static LIST: $crate::domain::DomainList<$name> = $crate::domain::DomainList::new();
                &LIST
            }
            #[inline(always)]
            fn get_thread_local_node() -> ::std::option::Option<$crate::domain::DomainNodeRef<Self>> {
                $name::local_key().with(::std::cell::Cell::get)
            }
            unsafe fn set_thread_local_node(node: Option<$crate::domain::DomainNodeRef<Self>>) {
                struct NodeGuard;
                impl Drop for NodeGuard {
                    fn drop(&mut self) {
                        <$name as $crate::domain::Domain>::release_thread_local_node();
                    }
                }
                ::std::thread_local! {
                    static GUARD: NodeGuard = const { NodeGuard };
                }
                $name::local_key().with(|cell| cell.set(node));
                GUARD.try_with(|_| ()).ok();
            }
        }
    };
}

/// Declare a domain using POSIX pthread API.
///
/// # Examples
///
/// ```rust
/// hazarc::pthread_domain!(pub(crate) MyDomain(2)); // 2 borrow slots
/// ```
///
/// # Unsafe use
///
/// Pthread thread-local key requires an initial call to `pthread_key_create`, often synchronized
/// with `pthread_once`. This is done in every `Domain` method of the generated domain, with a
/// runtime cost.
///
/// It is possible to declare an unsafe domain skipping this call. However, it requires to call
/// the generated `pthread_key_create` before any use of it.
///
/// ```rust
/// // Requires to call `MyUnsafeDomain::pthread_key_create` before any use of it.
/// hazarc::pthread_domain!(pub(crate) MyUnsafeDomain(2); unsafe { Self::pthread_key_already_created() });
/// ```
///
/// *This convoluted syntax purpose is to trigger unsafe-related lints,
/// like `clippy::undocumented_unsafe_blocks`. Any other expression than
/// `Self::Self::pthread_key_create()` and `Self::pthread_key_already_created()`
/// will trigger a runtime panic.*
#[cfg(feature = "pthread-domain")]
#[macro_export]
macro_rules! pthread_domain {
    ($(#[$attrs:meta])* $vis:vis $name:ident($borrow_slot_count:expr)) => {
        $crate::pthread_domain!($(#[$attrs])* $vis $name($borrow_slot_count); Self::pthread_key_create());
    };
    ($(#[$attrs:meta])* $vis:vis $name:ident($borrow_slot_count:expr); $init:expr) => {
        $(#[$attrs])*
        #[derive(Debug)]
        $vis struct $name;
        impl $name {
            #[doc(hidden)]
            #[inline(always)]
            fn key() -> *mut $crate::libc::pthread_key_t {
                static mut KEY: ::core::mem::MaybeUninit<$crate::libc::pthread_key_t> = core::mem::MaybeUninit::uninit();
                (&raw mut KEY).cast()
            }
            #[inline(always)]
            pub unsafe fn pthread_key_already_created() -> impl ::core::any::Any {
                pub struct Created;
                Created
            }
            #[inline]
            pub fn pthread_key_create() -> impl ::core::any::Any {
                unsafe extern "C" fn make_key() {
                    unsafe extern "C" fn release_node(ptr: *mut $crate::libc::c_void) {
                        if let Some(ptr) = ::core::ptr::NonNull::new(ptr) {
                            let node = unsafe { $crate::domain::DomainNodeRef::from_raw(ptr.cast()) };
                            unsafe { <$name as $crate::domain::Domain>::static_list().release_node(node) };
                        }
                    }
                    unsafe { $crate::libc::pthread_key_create($name::key(), Some(release_node)) };
                }
                static mut KEY_ONCE: $crate::libc::pthread_once_t = $crate::libc::PTHREAD_ONCE_INIT;
                #[allow(clippy::missing_transmute_annotations)] // signature is different across platforms
                unsafe { $crate::libc::pthread_once(&raw mut KEY_ONCE, ::core::mem::transmute(make_key as unsafe extern "C" fn())) };
                unsafe { Self::pthread_key_already_created() }
            }
        }
        unsafe impl $crate::domain::Domain for $name {
            const BORROW_SLOT_COUNT: usize = $borrow_slot_count;
            #[inline(always)]
            fn static_list() -> &'static $crate::domain::DomainList<Self> {
                static LIST: $crate::domain::DomainList<$name> = $crate::domain::DomainList::new();
                &LIST
            }
            #[inline(always)]
            fn get_thread_local_node() -> ::core::option::Option<$crate::domain::DomainNodeRef<Self>> {
                fn type_id<T: ::core::any::Any>(_: T) -> ::core::any::TypeId {
                    ::core::any::TypeId::of::<T>()
                }
                assert_eq!(type_id($init), type_id(unsafe { Self::pthread_key_already_created() }));
                let ptr = unsafe { ::core::ptr::NonNull::new($crate::libc::pthread_getspecific(*Self::key())) }?;
                Some(unsafe { $crate::domain::DomainNodeRef::from_raw(ptr.cast()) })
            }
            unsafe fn set_thread_local_node(node: Option<$crate::domain::DomainNodeRef<Self>>) {
                fn type_id<T: ::core::any::Any>(_: T) -> ::core::any::TypeId {
                    ::core::any::TypeId::of::<T>()
                }
                assert_eq!(type_id($init), type_id(unsafe { Self::pthread_key_already_created() }));
                let node_ptr = node.map_or(::core::ptr::null(), |n| n.into_raw().as_ptr().cast());
                unsafe { $crate::libc::pthread_setspecific(*$name::key(), node_ptr) };
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::domain::{Domain, ListAccessGuard};

    #[test]
    fn node_reuse() {
        #[cfg(feature = "pthread-domain")]
        pthread_domain!(TestDomain(2));
        #[cfg(not(feature = "pthread-domain"))]
        domain!(TestDomain(2));
        let node_addr = || (TestDomain::get_or_acquire_thread_local_node().into_raw()).addr();
        std::thread::scope(|s| {
            let _guard = ListAccessGuard::new(TestDomain::static_list()); // prevent gc
            let node1 = s.spawn(node_addr).join().unwrap();
            let thread2 = s.spawn(node_addr);
            let thread3 = s.spawn(node_addr);
            let node2 = thread2.join().unwrap();
            let node3 = thread3.join().unwrap();
            assert!(node1 == node2 || node1 == node3);
        });
    }

    #[test]
    fn reserve() {
        #[cfg(feature = "pthread-domain")]
        pthread_domain!(TestDomain(2));
        #[cfg(not(feature = "pthread-domain"))]
        domain!(TestDomain(2));
        std::thread::scope(|s| {
            let _guard = ListAccessGuard::new(TestDomain::static_list()); // prevent gc
            let thread = s.spawn(|| {
                TestDomain::get_or_acquire_thread_local_node();
            });
            TestDomain::static_list().reserve(4);
            thread.join().unwrap();
            assert_eq!(TestDomain::static_list().nodes().count(), 4);
        });
    }

    #[test]
    fn deallocation() {
        #[cfg(feature = "pthread-domain")]
        pthread_domain!(TestDomain(2));
        #[cfg(not(feature = "pthread-domain"))]
        domain!(TestDomain(2));
        let barrier = std::sync::Barrier::new(2);
        let guard = ListAccessGuard::new(TestDomain::static_list()); // prevent gc
        std::thread::scope(|s| {
            s.spawn(|| {
                TestDomain::get_or_acquire_thread_local_node();
                barrier.wait();
                // It seems TLS can be destroyed after the thread has been joined...
                TestDomain::release_thread_local_node();
            });
            s.spawn(|| {
                TestDomain::get_or_acquire_thread_local_node();
                barrier.wait();
                TestDomain::release_thread_local_node();
            });
        });
        assert_eq!(TestDomain::static_list().nodes().count(), 2);
        drop(guard);
        #[cfg(feature = "domain-gc")]
        assert_eq!(TestDomain::static_list().nodes().count(), 0);
        unsafe { TestDomain::static_list().deallocate() };
        assert_eq!(TestDomain::static_list().nodes().count(), 0);
    }
}

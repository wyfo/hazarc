#![allow(dead_code, unused_imports)]

#[cfg(all(loom, not(debug_assertions)))]
compile_error!("loom tests requires debug_assertions enabled");

#[cfg(not(loom))]
pub use std::{cell::Cell, sync::atomic::fence, sync::Arc, thread};

use hazarc::domain::Domain;
#[cfg(loom)]
pub use loom::{cell::Cell, sync::atomic::fence, sync::Arc};

/// Wraps `loom::model`, releasing the main thread's node of domain `D` at the end of each
/// iteration: loom drops lazy statics before main thread-locals, so a node acquired by a load in
/// the main thread must be released first.
///
/// Loom doesn't support `SeqCst` operations, so domains without asymmetric barriers, which rely
/// on them, are skipped.
#[cfg_attr(not(loom), allow(clippy::extra_unused_type_parameters))]
pub fn model<D: Domain>(f: impl Fn() + Sync + Send + 'static) {
    #[cfg(not(loom))]
    f();
    #[cfg(loom)]
    if std::any::TypeId::of::<D::Synchronization>()
        != std::any::TypeId::of::<hazarc::domain::Sequential>()
    {
        use hazarc::domain::DomainExt;
        loom::model(move || {
            f();
            D::release_thread_local_node();
        });
    }
}

/// Never frees memory, so addresses are never reused: the allocator state is not reset between
/// loom iterations, so address reuse, e.g. a new Arc allocated at the address of a dropped one,
/// would make the model non-deterministic.
#[cfg(loom)]
const _: () = {
    use std::alloc::{GlobalAlloc, Layout, System};

    struct LeakingAllocator;

    unsafe impl GlobalAlloc for LeakingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
    }

    #[global_allocator]
    static ALLOCATOR: LeakingAllocator = LeakingAllocator;
};

#[cfg(loom)]
pub mod thread {
    use std::{
        cell::RefCell,
        marker::PhantomData,
        panic::{catch_unwind, AssertUnwindSafe},
    };

    use loom::thread::JoinHandle;
    pub use loom::thread::{spawn, yield_now};

    /// Type-erased thread result.
    trait ThreadResult {}
    impl<T> ThreadResult for T {}

    /// Thread result with its lifetime erased; it never outlives the scope, as all threads are
    /// joined, and their results dropped, before the end of the scope.
    type ErasedResult = std::thread::Result<Box<dyn ThreadResult + Send + 'static>>;

    #[derive(Default)]
    pub struct Scope<'env> {
        handles: RefCell<Vec<Option<JoinHandle<ErasedResult>>>>,
        dummy: loom::sync::Arc<loom::sync::atomic::AtomicUsize>,
        _env: PhantomData<&'env mut ()>,
    }

    impl Drop for Scope<'_> {
        fn drop(&mut self) {
            // Like `std::thread::scope`, results that were not taken are dropped after the join.
            for handle in self.handles.get_mut().drain(..).flatten() {
                if let Err(err) = handle.join().unwrap() {
                    std::panic::resume_unwind(err);
                }
            }
        }
    }

    impl<'env> Scope<'env> {
        pub fn spawn<T: Send + 'env>(
            &self,
            f: impl FnOnce() -> T + Send + 'env,
        ) -> ScopedJoinHandle<'_, 'env, T> {
            let mut handles = self.handles.borrow_mut();
            let handle_idx = handles.len();
            let dummy = self.dummy.clone();
            handles.push(Some(spawn(unsafe {
                core::mem::transmute::<
                    Box<dyn FnOnce() -> ErasedResult + Send + 'env>,
                    Box<dyn FnOnce() -> ErasedResult + Send + 'static>,
                >(Box::new(move || {
                    // https://github.com/tokio-rs/loom/issues/392
                    dummy.store(1, loom::sync::atomic::Ordering::Relaxed);
                    // https://github.com/tokio-rs/loom/issues/417
                    catch_unwind(AssertUnwindSafe(|| {
                        core::mem::transmute::<
                            Box<dyn ThreadResult + Send + 'env>,
                            Box<dyn ThreadResult + Send + 'static>,
                        >(Box::new(f()))
                    }))
                }))
            })));
            ScopedJoinHandle {
                scope: self,
                handle_idx,
                _result: PhantomData,
            }
        }
    }

    pub struct ScopedJoinHandle<'scope, 'env, T> {
        scope: &'scope Scope<'env>,
        handle_idx: usize,
        _result: PhantomData<T>,
    }

    impl<T> ScopedJoinHandle<'_, '_, T> {
        pub fn join(self) -> std::thread::Result<T> {
            let handle = self.scope.handles.borrow_mut()[self.handle_idx].take();
            let result = handle.unwrap().join().unwrap()?;
            // SAFETY: the result has been boxed from a `T` by the spawned thread
            Ok(*unsafe { Box::from_raw(Box::into_raw(result) as *mut T) })
        }
    }

    pub fn scope<'env, T>(f: impl FnOnce(&Scope<'env>) -> T) -> T {
        let scope = Scope::default();
        scope.dummy.store(1, loom::sync::atomic::Ordering::Relaxed);
        f(&scope)
    }
}

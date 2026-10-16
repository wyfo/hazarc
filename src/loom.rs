pub(crate) mod sync {
    pub(crate) mod atomic {
        #[cfg(not(any(loom, feature = "portable-atomic")))]
        pub(crate) use core::sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering};

        #[cfg(loom)]
        pub(crate) use loom::sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering};
        #[cfg(all(feature = "portable-atomic", not(loom)))]
        pub(crate) use portable_atomic::{fence, AtomicPtr, AtomicUsize, Ordering};
    }
}

pub(crate) trait AtomicPtrExt<T> {
    fn load_mut(&mut self) -> *mut T;
}

impl<T> AtomicPtrExt<T> for sync::atomic::AtomicPtr<T> {
    #[cfg(not(loom))]
    fn load_mut(&mut self) -> *mut T {
        *self.get_mut()
    }
    #[cfg(loom)]
    fn load_mut(&mut self) -> *mut T {
        self.with_mut(|ptr| *ptr)
    }
}

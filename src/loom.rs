#[cfg(not(any(loom, feature = "portable-atomic")))]
pub(crate) use core::sync;

#[cfg(loom)]
pub(crate) use loom::sync;

#[cfg(all(feature = "portable-atomic", not(loom)))]
pub(crate) mod sync {
    pub(crate) use portable_atomic as atomic;
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

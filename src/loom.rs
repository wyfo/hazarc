#[cfg(not(loom))]
pub(crate) use core::sync;

#[cfg(loom)]
pub(crate) use loom::sync;

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

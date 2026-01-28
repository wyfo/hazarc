use core::sync::atomic::{
    AtomicPtr, Ordering,
    Ordering::{Acquire, Relaxed, SeqCst},
};

use crate::NULL;

#[allow(clippy::missing_safety_doc)]
#[expect(private_bounds)]
pub unsafe trait LoadPolicy: PrivateLoadPolicy {}

pub(crate) trait PrivateLoadPolicy: 'static {
    type AtomicPtr: AdaptiveAtomicPtr<Ptr = Self::Ptr>;
    type Ptr: AdaptivePtr;
    fn check_concurrent_writers() -> bool {
        false
    }
}

#[derive(Debug)]
pub struct WaitFree;
unsafe impl LoadPolicy for WaitFree {}
impl PrivateLoadPolicy for WaitFree {
    type AtomicPtr = AtomicPtr<()>;
    type Ptr = *mut ();
}

#[derive(Debug)]
pub struct Adaptive;
unsafe impl LoadPolicy for Adaptive {}
impl PrivateLoadPolicy for LockFree {
    type AtomicPtr = AtomicPtr<()>;
    type Ptr = *mut ();
}

#[derive(Debug)]
pub struct LockFree;
unsafe impl LoadPolicy for LockFree {}
impl PrivateLoadPolicy for Adaptive {
    type AtomicPtr = FlaggedAtomicPtr;
    type Ptr = FlaggedPtr;
    fn check_concurrent_writers() -> bool {
        true
    }
}

pub(crate) trait AdaptiveAtomicPtr: Send + Sync {
    type Ptr: AdaptivePtr;
    const NULL: Self;
    fn new(ptr: *mut ()) -> Self;
    fn get_mut(&mut self) -> Self::Ptr;
    fn load(&self, ordering: Ordering) -> Self::Ptr;
    fn swap(&self, ptr: *mut ()) -> Self::Ptr;
    fn compare_exchange(&self, current: *mut (), new: *mut ()) -> Result<Self::Ptr, Self::Ptr>;
    fn finish_write(&self, ptr: *mut ());
}
pub(crate) trait AdaptivePtr: Copy + PartialEq + Into<*mut ()> {
    fn is_null(self) -> bool;
    fn has_concurrent_writers(self) -> bool;
}

impl AdaptiveAtomicPtr for AtomicPtr<()> {
    type Ptr = *mut ();
    const NULL: Self = Self::new(NULL);
    #[inline(always)]
    fn new(ptr: *mut ()) -> Self {
        Self::new(ptr)
    }
    #[inline(always)]
    fn get_mut(&mut self) -> Self::Ptr {
        *self.get_mut()
    }
    #[inline(always)]
    fn load(&self, ordering: Ordering) -> Self::Ptr {
        self.load(ordering)
    }
    #[inline(always)]
    fn swap(&self, ptr: *mut ()) -> Self::Ptr {
        self.swap(ptr, SeqCst)
    }
    #[inline(always)]
    fn compare_exchange(&self, current: *mut (), new: *mut ()) -> Result<Self::Ptr, Self::Ptr> {
        self.compare_exchange(current, new, SeqCst, Acquire)
    }
    #[inline(always)]
    fn finish_write(&self, _ptr: *mut ()) {}
}
impl AdaptivePtr for *mut () {
    #[inline(always)]
    fn is_null(self) -> bool {
        self.is_null()
    }
    #[inline(always)]
    fn has_concurrent_writers(self) -> bool {
        false
    }
}

const ONGOING_WRITE_FLAG: usize = 0b01;
const CONCURRENT_WRITES_FLAG: usize = 0b10;

#[derive(Clone, Copy)]
pub(crate) struct FlaggedPtr(*mut ());
impl AdaptivePtr for FlaggedPtr {
    #[inline(always)]
    fn is_null(self) -> bool {
        <*mut ()>::from(self).is_null()
    }
    #[inline(always)]
    fn has_concurrent_writers(self) -> bool {
        let addr = self.0.addr();
        debug_assert!(addr & CONCURRENT_WRITES_FLAG == 0 || addr & ONGOING_WRITE_FLAG != 0);
        addr & CONCURRENT_WRITES_FLAG != 0
    }
}
impl PartialEq for FlaggedPtr {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        <*mut ()>::from(*self) == <*mut ()>::from(*other)
    }
}
impl From<FlaggedPtr> for *mut () {
    #[inline(always)]
    fn from(value: FlaggedPtr) -> Self {
        (value.0).map_addr(|addr| addr & !(ONGOING_WRITE_FLAG | CONCURRENT_WRITES_FLAG))
    }
}

pub(crate) struct FlaggedAtomicPtr(AtomicPtr<()>);
impl AdaptiveAtomicPtr for FlaggedAtomicPtr {
    type Ptr = FlaggedPtr;
    const NULL: Self = Self(AtomicPtr::NULL);
    #[inline(always)]
    fn new(ptr: *mut ()) -> Self {
        Self(AtomicPtr::new(ptr))
    }
    #[inline(always)]
    fn get_mut(&mut self) -> Self::Ptr {
        FlaggedPtr(*self.0.get_mut())
    }
    #[inline(always)]
    fn load(&self, ordering: Ordering) -> Self::Ptr {
        FlaggedPtr(self.0.load(ordering))
    }
    fn swap(&self, new: *mut ()) -> Self::Ptr {
        let new = new.map_addr(|addr| addr | ONGOING_WRITE_FLAG);
        let swap = || {
            let new = new.map_addr(|addr| addr | CONCURRENT_WRITES_FLAG);
            FlaggedPtr(self.0.swap(new, SeqCst))
        };
        let ptr = self.0.load(SeqCst);
        if ptr.addr() & ONGOING_WRITE_FLAG != 0 {
            return swap();
        }
        match self.0.compare_exchange(ptr, new, SeqCst, Relaxed) {
            Ok(old) => FlaggedPtr(old),
            Err(_) => swap(),
        }
    }
    fn compare_exchange(&self, current: *mut (), new: *mut ()) -> Result<Self::Ptr, Self::Ptr> {
        let new = new.map_addr(|addr| addr | ONGOING_WRITE_FLAG);
        match self.0.compare_exchange(current, new, SeqCst, Acquire) {
            Err(c) if c.addr() & ONGOING_WRITE_FLAG != 0 => {
                (self.0).fetch_or(ONGOING_WRITE_FLAG | CONCURRENT_WRITES_FLAG, SeqCst);
                let current =
                    current.map_addr(|addr| addr | ONGOING_WRITE_FLAG | CONCURRENT_WRITES_FLAG);
                let new = new.map_addr(|addr| addr | CONCURRENT_WRITES_FLAG);
                self.0.compare_exchange(current, new, SeqCst, Relaxed)
            }
            res => res,
        }
        .map(FlaggedPtr)
        .map_err(FlaggedPtr)
    }
    fn finish_write(&self, old: *mut ()) {
        let ptr = self.0.load(SeqCst);
        if ptr.addr() == old.addr() | ONGOING_WRITE_FLAG
            && let Err(ptr) = self.0.compare_exchange(ptr, old, SeqCst, Relaxed)
        {
            debug_assert!(ptr.addr() & ONGOING_WRITE_FLAG != 0);
            debug_assert!(ptr.addr() & CONCURRENT_WRITES_FLAG != 0);
        }
    }
}

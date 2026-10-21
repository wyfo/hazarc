// 1.82: `addr_of` --> `&raw const`

#[allow(dead_code)]
pub(crate) trait OptionExt<T> {
    #[allow(clippy::wrong_self_convention)]
    fn is_none_or(self, f: impl FnOnce(T) -> bool) -> bool;
}

impl<T> OptionExt<T> for Option<T> {
    fn is_none_or(self, f: impl FnOnce(T) -> bool) -> bool {
        match self {
            None => true,
            Some(x) => f(x),
        }
    }
}

// 1.76: `Result::inspect`
#[allow(dead_code)]
pub(crate) trait ResultExt<T, E>: Sized {
    fn inspect<F: FnOnce(&T)>(self, f: F) -> Self;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn inspect<F: FnOnce(&T)>(self, f: F) -> Self {
        if let Ok(ref t) = self {
            f(t);
        }
        self
    }
}

#[allow(dead_code)]
pub(crate) trait StrictProvenance<T>: Sized + Copy {
    type Addr;
    fn addr(self) -> Self::Addr;
    fn with_addr(self, addr: Self::Addr) -> Self;
    fn map_addr(self, f: impl FnOnce(Self::Addr) -> Self::Addr) -> Self {
        self.with_addr(f(self.addr()))
    }
}

impl<T> StrictProvenance<T> for *const T {
    type Addr = usize;
    fn addr(self) -> Self::Addr {
        self as usize
    }
    fn with_addr(self, addr: Self::Addr) -> Self {
        let ptr_addr = self as isize;
        let dest_addr = addr as isize;
        let offset = dest_addr.wrapping_sub(ptr_addr);
        self.cast::<u8>().wrapping_offset(offset).cast()
    }
}

impl<T> StrictProvenance<T> for *mut T {
    type Addr = usize;
    fn addr(self) -> Self::Addr {
        self as usize
    }
    fn with_addr(self, addr: Self::Addr) -> Self {
        self.cast_const().with_addr(addr).cast_mut()
    }
}

// 1.84: `NonNull::addr`/`NonNull::with_addr`
impl<T> StrictProvenance<T> for ptr::NonNull<T> {
    type Addr = core::num::NonZeroUsize;
    fn addr(self) -> Self::Addr {
        // SAFETY: the pointer is not null
        unsafe { core::num::NonZeroUsize::new_unchecked(self.as_ptr().addr()) }
    }
    fn with_addr(self, addr: Self::Addr) -> Self {
        // SAFETY: the address is not zero
        unsafe { ptr::NonNull::new_unchecked(self.as_ptr().with_addr(addr.get())) }
    }
}

// 1.87: `offset_from_unsigned`
#[allow(dead_code)]
pub(crate) trait OffsetFromUnsigned<T> {
    unsafe fn offset_from_unsigned(self, origin: *const T) -> usize;
}

impl<T> OffsetFromUnsigned<T> for *const T {
    #[inline(always)]
    unsafe fn offset_from_unsigned(self, origin: *const T) -> usize {
        // SAFETY: same function contract
        let offset = unsafe { self.offset_from(origin) };
        unsafe { hint::assert_unchecked(offset >= 0) };
        offset as usize
    }
}

pub(crate) mod fmt {
    pub(crate) use core::fmt::*;

    // 1.93: `fmt::from_fn`
    pub(crate) fn from_fn<F: Fn(&mut Formatter<'_>) -> Result>(f: F) -> FromFn<F> {
        FromFn(f)
    }

    pub(crate) struct FromFn<F>(F);

    impl<F: Fn(&mut Formatter<'_>) -> Result> Debug for FromFn<F> {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            (self.0)(f)
        }
    }
}

pub(crate) mod hint {
    pub(crate) use core::hint::*;

    // 1.81: `hint::assert_unchecked`
    #[inline(always)]
    pub(crate) const unsafe fn assert_unchecked(cond: bool) {
        if !cond {
            unsafe { unreachable_unchecked() }
        }
    }

    // 1.95: `hint::cold_path`
    #[cold]
    #[inline(always)]
    pub(crate) const fn cold_path() {}
}

pub(crate) mod ptr {
    pub(crate) use core::ptr::*;

    pub(crate) const fn from_ref<T: ?Sized>(t: &T) -> *const T {
        t as _
    }

    pub(crate) const fn without_provenance_mut<T>(addr: usize) -> *mut T {
        null_mut::<u8>().wrapping_add(addr).cast()
    }
}

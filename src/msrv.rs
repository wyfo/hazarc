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
    #[allow(unstable_name_collisions)]
    fn with_addr(self, addr: Self::Addr) -> Self {
        self.cast_const().with_addr(addr).cast_mut()
    }
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

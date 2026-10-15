//! Reimplementation of `membarrier` crate, which can't be used because it depends unconditionally
//! on `windows-sys`.
//!
//! `membarrier2` crate has not been used instead, as it has an unconditional dependency on `ctor`.

#[inline]
pub(crate) fn heavy() {
    #[cfg(target_os = "linux")]
    linux::heavy();
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Threading::FlushProcessWriteBuffers();
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        io,
        sync::{
            atomic::{AtomicUsize, Ordering::SeqCst},
            Mutex, OnceLock, PoisonError,
        },
    };

    use crate::msrv::ptr;

    enum Strategy {
        Membarrier,
        /// The page used by the `mprotect` fallback; the mutex also serializes the barriers.
        Mprotect(Mutex<Page>),
    }

    #[derive(Clone, Copy)]
    struct Page(*mut libc::c_void, usize);
    // SAFETY: the page is only accessed through the mutex
    unsafe impl Send for Page {}

    #[inline]
    pub(super) fn heavy() {
        static STRATEGY: OnceLock<Strategy> = OnceLock::new();
        // Registers the process to the `membarrier` system call if it is supported, or
        // initializes the `mprotect` page otherwise.
        let strategy = STRATEGY.get_or_init(|| match register_membarrier() {
            true => Strategy::Membarrier,
            false => Strategy::Mprotect(Mutex::new(mprotect_page())),
        });
        match strategy {
            Strategy::Membarrier => membarrier(),
            Strategy::Mprotect(page) => mprotect_barrier(page),
        }
    }

    fn sys_membarrier(cmd: libc::c_int) -> libc::c_long {
        unsafe { libc::syscall(libc::SYS_membarrier, cmd, 0 as libc::c_int) }
    }

    fn register_membarrier() -> bool {
        let cmds = sys_membarrier(libc::MEMBARRIER_CMD_QUERY);
        let expedited = libc::MEMBARRIER_CMD_PRIVATE_EXPEDITED as libc::c_long;
        let register = libc::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED as libc::c_long;
        cmds >= 0
            && cmds & expedited != 0
            && cmds & register != 0
            && sys_membarrier(libc::MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED) >= 0
    }

    fn membarrier() {
        if sys_membarrier(libc::MEMBARRIER_CMD_PRIVATE_EXPEDITED) < 0 {
            panic!(
                "membarrier system call failed: {}",
                io::Error::last_os_error()
            );
        }
    }

    fn mprotect_page() -> Page {
        unsafe {
            // Fallback page size taken from `membarrier2` (`membarrier` asserts instead)
            let page_size = match libc::sysconf(libc::_SC_PAGESIZE) {
                size if size > 0 => size as usize,
                _ => 0x1000,
            };
            // Mapped readable and writable, as in `membarrier2`, as `mlock` fails on an
            // inaccessible page (`membarrier` maps it with `PROT_NONE`, so its `mlock`, whose
            // result is ignored, always fails)
            let page = libc::mmap(
                ptr::null_mut(),
                page_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            if page == libc::MAP_FAILED {
                panic!(
                    "mprotect barrier mmap failed: {}",
                    io::Error::last_os_error()
                );
            }
            assert_eq!(
                page as usize % page_size,
                0,
                "mprotect barrier page is not aligned"
            );
            // The page must stay in memory between the two `mprotect`, otherwise they would not
            // issue the expected inter-processor interrupts.
            // The result is checked as in `membarrier2` (`membarrier` ignores it)
            if libc::mlock(page, page_size) != 0 {
                panic!(
                    "mprotect barrier mlock failed: {}",
                    io::Error::last_os_error()
                );
            }
            Page(page, page_size)
        }
    }

    fn mprotect_barrier(page: &Mutex<Page>) {
        let guard = page.lock().unwrap_or_else(PoisonError::into_inner);
        let Page(page, page_size) = *guard;
        unsafe {
            if libc::mprotect(page, page_size, libc::PROT_READ | libc::PROT_WRITE) != 0 {
                panic!("mprotect barrier failed: {}", io::Error::last_os_error());
            }
            // The page must be dirty, so that the OS doesn't skip the TLB shootdown
            (*page.cast::<AtomicUsize>()).fetch_add(1, SeqCst);
            // Removing access triggers a TLB shootdown, i.e. inter-processor interrupts
            // flushing the store buffers of every core running the process
            if libc::mprotect(page, page_size, libc::PROT_NONE) != 0 {
                panic!("mprotect barrier failed: {}", io::Error::last_os_error());
            }
        }
    }
}

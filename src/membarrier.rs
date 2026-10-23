//! Reimplementation of `membarrier` crate, which can't be used because it depends unconditionally
//! on `windows-sys`.
//!
//! `membarrier2` crate has not been used instead, as it has an unconditional dependency on `ctor`.
//! Its support check, executed at load time, is replaced by `AsymmetricBarriers::init`, called in
//! `AtomicArc` constructors.

/// Process-wide memory barriers, with compiler fences as light barriers.
///
/// Loads' critical path only executes compiler fences instead of memory barriers/atomic RMW
/// operations, at the cost of a process-wide memory barrier in writes, e.g. a `membarrier` system
/// call on Linux. It makes loads faster, but writes significantly costlier, impacting all the
/// program's threads. Operations are also no longer sequentially consistent, see
/// [memory ordering](crate#memory-ordering).
///
/// It should not be used in combination with [`Cache`](crate::Cache), as its loads don't use any
/// barrier as long as the `AtomicArc` is not updated, while writes still pay for the
/// process-wide memory barrier.
///
/// # Compatibility
///
/// | Platform        | Process-wide memory barrier                               |
/// |-----------------|-----------------------------------------------------------|
/// | Linux, Android  | `membarrier` system call, with `mprotect` fallback on x86 |
/// | FreeBSD         | `membarrier` system call (FreeBSD 14.1+)                  |
/// | Windows         | `FlushProcessWriteBuffers`                                |
/// | Apple platforms | `thread_get_register_pointer_values` on every thread      |
///
/// If the `membarrier` system call is not available, barriers fall back to `fence(SeqCst)`, except
/// on x86 Linux/Android with the `mprotect` fallback. Selecting the fallback at runtime adds a
/// `Relaxed` atomic check to the loads' critical path (except on x86 Linux/Android), unless
/// compiled with `--cfg hazarc_assume_membarrier`, in which case writes panic if `membarrier` is
/// not available.
///
/// On ARM, process-wide memory barriers are only used on AArch64 with `rcpc` target feature
/// (enabled by default on macOS/iOS).
///
/// On other platforms, `AtomicArc` operations are sequentially consistent, like with
/// [`Sequential`](crate::domain::Sequential).
///
/// # Examples
///
/// ```rust
/// hazarc::domain!(pub(crate) MyDomain(synchronization = hazarc::domain::Membarrier));
/// ```
#[derive(Debug)]
pub struct Membarrier;

// SAFETY: a process-wide memory barrier synchronizes with compiler fences; with miri/loom, both
// barriers are `fence(SeqCst)`
// Platforms providing a process-wide memory barrier, possibly not supported at runtime, e.g.
// missing `membarrier` system call on old FreeBSD kernels, in which case barriers fall back to
// `fence(SeqCst)`.
// On ARM, only with RCpc (`ldapr`), as the acquire load of the critical path is otherwise already
// ordered after its release store (`ldar` on AArch64, `dmb` on 32-bit ARM), making process-wide
// memory barriers useless.
#[cfg(all(
    any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "windows",
        target_vendor = "apple"
    ),
    not(target_arch = "arm"),
    any(not(target_arch = "aarch64"), target_feature = "rcpc"),
))]
unsafe impl crate::domain::AsymmetricBarriers for Membarrier {
    /// Initializes the process-wide memory barrier if needed. Before this call, light barrier may
    /// default to `fence(SeqCst)` if support is not assumed.
    #[cfg(not(any(miri, loom)))]
    #[inline]
    fn init() {
        #[cfg(all(
            any(
                all(
                    any(target_os = "linux", target_os = "android"),
                    not(any(target_arch = "x86", target_arch = "x86_64"))
                ),
                target_os = "freebsd"
            ),
            not(hazarc_assume_membarrier)
        ))]
        unix::strategy();
    }

    #[cfg(not(any(miri, loom)))]
    #[inline(always)]
    fn light_barrier() {
        // Without `membarrier` system call, the barrier falls back to `fence(SeqCst)`, unless there
        // is the `mprotect` fallback (only on x86).
        // Relaxed, as the flag is set before any `AtomicArc` is constructed, except with const
        // constructors. Reading a stale `false` only executes a superfluous fence, while `true` can
        // only be read after the initialization, so `heavy_barrier` uses the `membarrier` system
        // call.
        // With `hazarc_assume_membarrier` cfg, the check is bypassed, and `heavy_barrier` panics
        // instead of falling back.
        #[cfg(all(
            any(
                all(
                    any(target_os = "linux", target_os = "android"),
                    not(any(target_arch = "x86", target_arch = "x86_64"))
                ),
                target_os = "freebsd"
            ),
            not(hazarc_assume_membarrier)
        ))]
        if !unix::MEMBARRIER_SUPPORTED.load(core::sync::atomic::Ordering::Relaxed) {
            crate::msrv::hint::cold_path();
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            return;
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(any(miri, loom))]
    #[inline(always)]
    fn light_barrier() {
        crate::loom::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(not(any(miri, loom)))]
    #[inline]
    fn heavy_barrier() {
        #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
        unix::heavy();
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::System::Threading::FlushProcessWriteBuffers();
        }
        #[cfg(target_vendor = "apple")]
        apple::heavy();
    }

    #[cfg(any(miri, loom))]
    #[inline(always)]
    fn heavy_barrier() {
        crate::loom::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    }
}

// On unsupported platforms, `Membarrier` is equivalent to `Sequential`.
#[cfg(not(all(
    any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "windows",
        target_vendor = "apple"
    ),
    not(target_arch = "arm"),
    any(not(target_arch = "aarch64"), target_feature = "rcpc"),
)))]
impl crate::domain::private::Synchronization for Membarrier {
    const ASYMMETRIC_BARRIERS: bool = false;
    fn init_barriers() {
        unreachable!("no barriers")
    }
    fn light_barrier() {
        unreachable!("no barriers")
    }
    fn heavy_barrier() {
        unreachable!("no barriers")
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "android", target_os = "freebsd"),
    not(target_arch = "arm"),
    any(not(target_arch = "aarch64"), target_feature = "rcpc"),
    not(any(miri, loom)),
))]
mod unix {
    use std::{
        io,
        sync::{
            atomic::{fence, Ordering::SeqCst},
            OnceLock,
        },
    };

    /// Set if the `membarrier` system call is supported, see `Membarrier::light_barrier`.
    #[cfg(not(any(
        all(
            any(target_os = "linux", target_os = "android"),
            any(target_arch = "x86", target_arch = "x86_64")
        ),
        hazarc_assume_membarrier
    )))]
    pub(super) static MEMBARRIER_SUPPORTED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    pub(super) enum Strategy {
        Membarrier,
        /// The page used by the `mprotect` fallback; the mutex also serializes the barriers.
        #[cfg(all(
            any(target_os = "linux", target_os = "android"),
            any(target_arch = "x86", target_arch = "x86_64")
        ))]
        Mprotect(std::sync::Mutex<mprotect::Page>),
        /// `fence(SeqCst)` fallback, paired with the one of `Membarrier::light_barrier`.
        Fence,
    }

    /// Registers the process to the `membarrier` system call if it is supported, or initializes
    /// the fallback otherwise.
    pub(super) fn strategy() -> &'static Strategy {
        static STRATEGY: OnceLock<Strategy> = OnceLock::new();
        STRATEGY.get_or_init(|| {
            if register_membarrier() {
                #[cfg(not(any(
                    all(
                        any(target_os = "linux", target_os = "android"),
                        any(target_arch = "x86", target_arch = "x86_64")
                    ),
                    hazarc_assume_membarrier
                )))]
                MEMBARRIER_SUPPORTED.store(true, std::sync::atomic::Ordering::Relaxed);
                return Strategy::Membarrier;
            }
            #[cfg(all(
                any(target_os = "linux", target_os = "android"),
                any(target_arch = "x86", target_arch = "x86_64")
            ))]
            return Strategy::Mprotect(std::sync::Mutex::new(mprotect::page()));
            #[cfg(hazarc_assume_membarrier)]
            #[allow(unreachable_code)]
            {
                panic!("membarrier system call is not supported");
            }
            #[allow(unreachable_code)]
            Strategy::Fence
        })
    }

    #[inline]
    pub(super) fn heavy() {
        // The strategy is always loaded, even if `init` has already been called, as const
        // constructed `AtomicArc` don't call it, so the flag may not be visible.
        match strategy() {
            Strategy::Membarrier => membarrier(),
            #[cfg(all(
                any(target_os = "linux", target_os = "android"),
                any(target_arch = "x86", target_arch = "x86_64")
            ))]
            Strategy::Mprotect(page) => mprotect::barrier(page),
            Strategy::Fence => fence(SeqCst),
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    use libc::{
        SYS_membarrier, MEMBARRIER_CMD_PRIVATE_EXPEDITED, MEMBARRIER_CMD_QUERY,
        MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED,
    };

    // `libc` doesn't define these constants for FreeBSD (`membarrier` is supported since 14.1)
    // https://github.com/freebsd/freebsd-src/blob/041e9eb1ae094a81e55fbcaba37eb2ac194658cc/sys/sys/membarrier.h
    // https://github.com/freebsd/freebsd-src/blob/041e9eb1ae094a81e55fbcaba37eb2ac194658cc/sys/sys/syscall.h#L525
    #[cfg(target_os = "freebsd")]
    #[allow(non_upper_case_globals)]
    const SYS_membarrier: libc::c_int = 584;
    #[cfg(target_os = "freebsd")]
    const MEMBARRIER_CMD_QUERY: libc::c_int = 0x00000000;
    #[cfg(target_os = "freebsd")]
    const MEMBARRIER_CMD_PRIVATE_EXPEDITED: libc::c_int = 0x00000008;
    #[cfg(target_os = "freebsd")]
    const MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED: libc::c_int = 0x00000010;

    fn sys_membarrier(cmd: libc::c_int) -> libc::c_long {
        // The return type of `syscall` is `c_long` on Linux, `c_int` on FreeBSD
        #[allow(clippy::useless_conversion)]
        unsafe {
            libc::syscall(SYS_membarrier, cmd, 0 as libc::c_int).into()
        }
    }

    fn register_membarrier() -> bool {
        let cmds = sys_membarrier(MEMBARRIER_CMD_QUERY);
        let expedited = MEMBARRIER_CMD_PRIVATE_EXPEDITED as libc::c_long;
        let register = MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED as libc::c_long;
        cmds >= 0
            && cmds & expedited != 0
            && cmds & register != 0
            && sys_membarrier(MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED) >= 0
    }

    fn membarrier() {
        if sys_membarrier(MEMBARRIER_CMD_PRIVATE_EXPEDITED) < 0 {
            panic!(
                "membarrier system call failed: {}",
                io::Error::last_os_error()
            );
        }
    }

    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    mod mprotect {
        use std::{
            io,
            sync::{
                atomic::{AtomicUsize, Ordering::SeqCst},
                Mutex, PoisonError,
            },
        };

        use crate::msrv::ptr;

        #[derive(Clone, Copy)]
        pub(crate) struct Page(*mut libc::c_void, usize);
        // SAFETY: the page is only accessed through the mutex
        unsafe impl Send for Page {}

        pub(super) fn page() -> Page {
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
                // The page must stay in memory between the two `mprotect`, otherwise they would
                // not issue the expected inter-processor interrupts.
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

        pub(super) fn barrier(page: &Mutex<Page>) {
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
}

/// Taken from `membarrier2`, itself taken from .NET runtime:
/// <https://github.com/dotnet/runtime/blob/c290dc12ccd792d6cb431f47efc46df8cb9a7747/src/native/minipal/memorybarrierprocesswide.c#L157>
#[cfg(all(
    target_vendor = "apple",
    not(target_arch = "arm"),
    any(not(target_arch = "aarch64"), target_feature = "rcpc"),
    not(any(miri, loom)),
))]
mod apple {
    use std::{ffi::CStr, mem::MaybeUninit};

    use mach2::{
        error::{err_get_code, err_get_sub, err_get_system},
        kern_return::{kern_return_t, KERN_SUCCESS},
        mach_port::mach_port_deallocate,
        mach_types::thread_act_array_t,
        message::mach_msg_type_number_t,
        task::task_threads,
        thread_state::thread_get_register_pointer_values,
        traps::mach_task_self,
    };

    /// Requests the register values of every thread of the task, which forces them to execute a
    /// memory barrier.
    pub(super) fn heavy() {
        unsafe {
            let mut threads = MaybeUninit::<thread_act_array_t>::uninit();
            let mut threads_len = MaybeUninit::<mach_msg_type_number_t>::uninit();
            let machret = task_threads(
                mach_task_self(),
                threads.as_mut_ptr(),
                threads_len.as_mut_ptr(),
            );
            if machret != KERN_SUCCESS {
                panic_with_msg(machret, "task_threads failed");
            }
            let threads = threads.assume_init();
            let threads_len = threads_len.assume_init() as usize;
            assert!(!threads.is_null(), "threads ptr is null");
            const REGISTER_SIZE: usize = 128;
            let mut register_values = [MaybeUninit::<usize>::uninit(); REGISTER_SIZE];
            let mut sp = MaybeUninit::<usize>::uninit();
            // Memory is reclaimed before panicking
            let mut first_err = None;
            for &thread in std::slice::from_raw_parts(threads, threads_len) {
                let mut registers = REGISTER_SIZE;
                let machret = thread_get_register_pointer_values(
                    thread,
                    sp.as_mut_ptr(),
                    &mut registers,
                    register_values.as_mut_ptr().cast(),
                );
                // Other errors, e.g. for a terminated thread, are ignored, as in `membarrier2`
                if machret == libc::KERN_INSUFFICIENT_BUFFER_SIZE {
                    first_err =
                        first_err.or(Some((machret, "thread_get_register_pointer_values failed")));
                }
                let machret = mach_port_deallocate(mach_task_self(), thread);
                if machret != KERN_SUCCESS {
                    first_err = first_err.or(Some((machret, "mach_port_deallocate failed")));
                }
            }
            let machret = libc::vm_deallocate(
                mach_task_self(),
                threads as libc::vm_address_t,
                (threads_len * core::mem::size_of::<libc::thread_act_t>()) as libc::vm_size_t,
            );
            if machret != KERN_SUCCESS {
                first_err = first_err.or(Some((machret, "vm_deallocate failed")));
            }
            if let Some((machret, msg)) = first_err {
                panic_with_msg(machret, msg);
            }
        }
    }

    #[cold]
    fn panic_with_msg(machret: kern_return_t, msg: &'static str) -> ! {
        let err_msg = unsafe { CStr::from_ptr(libc::mach_error_string(machret)) };
        let (sys, sub, code) = (
            err_get_system(machret),
            err_get_sub(machret),
            err_get_code(machret),
        );
        panic!(
            "{msg}: {} (os error [{sys:#x}|{sub:#x}|{code:#x}])",
            err_msg.to_string_lossy()
        )
    }
}

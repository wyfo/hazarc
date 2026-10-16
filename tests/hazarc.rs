use std::{mem::ManuallyDrop, ptr};

use hazarc::{
    arc::ArcPtr,
    atomic::{ArcPtrBorrow, AtomicArcPtr, AtomicOptionArcPtr},
    domain::Domain,
    write_policy::{Concurrent, Serialized, WritePolicy},
};
use rstest::rstest;

use crate::loom::{model, thread, Arc, Cell};

mod loom;

type AtomicArc<T, D, W> = AtomicArcPtr<Arc<T>, D, W>;
type AtomicOptionArc<T, D, W> = AtomicOptionArcPtr<Arc<T>, D, W>;

/// Creates isolated domains per tests
macro_rules! d {
    ($slots:literal) => {{
        hazarc::domain!(TestDomain($slots));
        TestDomain
    }};
}

/// Creates isolated domains per tests, using asymmetric barriers (`membarrier` if the feature is
/// enabled, `fence(SeqCst)` otherwise).
macro_rules! dm {
    ($slots:literal) => {{
        hazarc::__domain!(TestDomain($slots, true));
        TestDomain
    }};
}

#[rstest]
fn is_outdated<D: Domain, W: WritePolicy>(
    #[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let atomic_arc = AtomicArc::<usize, D, W>::new(Arc::new(0));
        let borrow = atomic_arc.load();
        assert!(!borrow.is_outdated(&atomic_arc));
        let old = atomic_arc.swap(Arc::new(1));
        assert!(borrow.is_outdated(&atomic_arc));
        atomic_arc.store(old);
        assert!(!borrow.is_outdated(&atomic_arc));
        drop(borrow);
    });
}

#[rstest]
fn concurrent_reads<D: Domain, W: WritePolicy>(
    #[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let check_borrow = |b: &usize| assert!([0, 1, 2].contains(b));
        let atomic_arc = AtomicArc::<usize, D, W>::new(Arc::new(0));
        thread::scope(|s| {
            s.spawn(|| check_borrow(&atomic_arc.load()));
            s.spawn(|| check_borrow(&atomic_arc.load()));
            s.spawn(|| {
                atomic_arc.store(Arc::new(1));
                atomic_arc.store(Arc::new(2));
            });
        });
    });
}

#[rstest]
fn concurrent_reads_other_atomic_arc<D: Domain, W: WritePolicy>(
    #[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let check_borrow = |b: &_| assert!([0, 1].contains(b));
        let atomic_arc = AtomicArc::<usize, D, W>::new(Arc::new(0));
        let other = AtomicArc::<usize, D, W>::new(Arc::new(42));
        thread::scope(|s| {
            s.spawn(|| {
                let borrow1 = atomic_arc.load();
                let other_borrow = other.load();
                let borrow2 = atomic_arc.load();
                check_borrow(&borrow1);
                assert_eq!(**other_borrow, 42);
                check_borrow(&borrow2);
            });
            s.spawn(|| atomic_arc.store(Arc::new(1)));
        });
    });
}

#[rstest]
fn concurrent_writes<D: Domain, W: WritePolicy>(
    #[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let check_borrow = |b: &_| assert!([0, 1, 2].contains(b));
        let atomic_arc = AtomicArc::<usize, D, W>::new(Arc::new(0));
        thread::scope(|s| {
            s.spawn(|| {
                let swapped = atomic_arc.swap(Arc::new(1));
                assert!(*swapped == 0 || *swapped == 2);
            });
            s.spawn(|| {
                let swapped = atomic_arc.swap(Arc::new(2));
                assert!(*swapped == 0 || *swapped == 1);
            });
            s.spawn(|| {
                let guard = atomic_arc.load();
                check_borrow(&guard);
                check_borrow(&atomic_arc.load());
                check_borrow(&atomic_arc.load());
            });
        });
    });
}

#[rstest]
fn concurrent_writes_option<D: Domain, W: WritePolicy>(
    #[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let check_borrow = |b: &Option<ArcPtrBorrow<Arc<_>>>| {
            assert!([Some(0), Some(1), None].contains(&b.as_ref().map(|b| ***b)));
        };
        let atomic_arc = AtomicOptionArc::<usize, D, W>::new(Some(Arc::new(0)));
        thread::scope(|s| {
            s.spawn(|| {
                let swapped = atomic_arc.swap(Some(Arc::new(1)));
                assert!(swapped.as_deref() == Some(&0) || swapped.is_none());
            });
            s.spawn(|| {
                let swapped = atomic_arc.swap(None);
                assert!(swapped.as_deref() == Some(&1) || swapped.as_deref() == Some(&0));
            });
            s.spawn(|| {
                let guard = atomic_arc.load();
                check_borrow(&guard);
                check_borrow(&atomic_arc.load());
                check_borrow(&atomic_arc.load());
            });
        });
    });
}

#[rstest]
fn drop_atomic_arc_with_active_borrow<D: Domain, W: WritePolicy>(
    #[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let atomic_arc = AtomicArc::<usize, D, W>::new(Arc::new(0));
        let borrow = atomic_arc.load();
        drop(atomic_arc);
        drop(borrow);
    });
}

#[rstest]
fn drop_borrow_in_another_thread<D: Domain, W: WritePolicy>(
    #[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let atomic_arc = AtomicOptionArc::<usize, D, W>::new(Some(Arc::new(0)));
        thread::scope(|s| {
            let borrow = s.spawn(|| atomic_arc.load());
            s.spawn(|| atomic_arc.store(None));
            drop(borrow.join().unwrap());
        });
    });
}

/// A read-only cell written on drop, to check that borrow happens before drop.
struct WriteOnDrop(Cell<usize>);
// SAFETY: the cell is only written in the destructor
unsafe impl Sync for WriteOnDrop {}
impl WriteOnDrop {
    fn new() -> Self {
        Self(Cell::new(0))
    }
    fn read(&self) -> usize {
        self.0.get()
    }
}
impl Drop for WriteOnDrop {
    fn drop(&mut self) {
        self.0.set(usize::MAX);
    }
}

/// The use of a borrow must happen before the drop of the swapped Arc by the writer.
#[rstest]
fn borrow_happens_before_drop<D: Domain, W: WritePolicy>(
    #[values(d!(1), dm!(1))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let atomic_arc = AtomicArc::<WriteOnDrop, D, W>::new(Arc::new(WriteOnDrop::new()));
        thread::scope(|s| {
            s.spawn(|| assert_eq!(atomic_arc.load().read(), 0));
            s.spawn(|| atomic_arc.store(Arc::new(WriteOnDrop::new())));
        });
    });
}

#[rstest]
fn seq_cst_ordering<D: Domain, W: WritePolicy>(
    #[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let x = AtomicOptionArc::<(), D, W>::new(None);
        let y = AtomicOptionArc::<(), D, W>::new(None);
        thread::scope(|s| {
            let a = s.spawn(|| {
                x.store(Some(Arc::new(())));
                y.load()
            });
            let b = s.spawn(|| {
                y.store(Some(Arc::new(())));
                x.load()
            });
            assert!(a.join().unwrap().is_some() || b.join().unwrap().is_some());
        });
    });
}

#[rstest]
fn fetch_and_add<D: Domain>(#[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D) {
    model::<D>(|| {
        let atomic_arc = AtomicArc::<usize, D, Concurrent>::new(Arc::new(0));
        thread::scope(|s| {
            s.spawn(|| atomic_arc.update(|i| **i + 1));
            s.spawn(|| atomic_arc.update(|i| **i + 1));
        });
        assert_eq!(**atomic_arc.load(), 2);
    });
}

/// Consecutive loads must be monotonic with `Concurrent`.
///
/// See `non_monotonic_loads` for the interleaving making these loads non-monotonic with
/// `Serialized`.
#[rstest]
fn monotonic_loads<D: Domain, W: WritePolicy>(
    #[values(d!(1), dm!(1))] _domain: D,
    #[values(Concurrent)] _policy: W,
) {
    model::<D>(|| {
        let atomic_arc = AtomicArc::<usize, D, W>::new(Arc::new(0));
        thread::scope(|s| {
            s.spawn(|| atomic_arc.store(Arc::new(1)));
            s.spawn(|| {
                atomic_arc.store(Arc::new(2));
                atomic_arc.store(Arc::new(3));
            });
            s.spawn(|| {
                let a1 = **atomic_arc.load();
                let a2 = atomic_arc.load();
                let a3 = **atomic_arc.load();
                assert!(a1 == **a2 || a3 != a1, "non-monotonic loads");
            });
        });
    });
}

/// Consecutive loads may be non-monotonic with `Serialized` and concurrent writes.
///
/// Interleaving of `monotonic_loads` making the loads non-monotonic:
/// 1. the first load writes its slot, then writer A swaps;
/// 2. the first load goes through the slow path, writes `LOADING` in its slot, then A helps it,
///    materializing its helper value `1`, and is delayed before scanning the next slot;
/// 3. writer B stores `2`, which the second load borrows; it is kept alive, so the third load
///    uses the next slot;
/// 4. the third load writes its slot, then B stores `3`;
/// 5. the third load goes through the slow path, writes `LOADING` in its slot, then the delayed
///    A helps it with its outdated helper value `1`.
///
/// Ignored, as this interleaving requires about 6 preemptions, which is out of reach for loom.
#[cfg(loom)]
#[rstest]
#[ignore]
#[should_panic(expected = "non-monotonic loads")]
fn non_monotonic_loads<D: Domain>(#[values(dm!(1))] domain: D) {
    monotonic_loads(domain, Serialized);
}

/// An `Arc<usize>` whose value must always be `TAG`, emulating a distinct pointer type per tag.
///
/// Panics in drop if the value doesn't match (because of unsafe manipulation).
#[derive(Clone)]
struct Tagged<const TAG: usize>(Arc<usize>);
impl<const TAG: usize> Tagged<TAG> {
    fn new() -> Self {
        Self(Arc::new(TAG))
    }
    fn try_retag<const NEW_TAG: usize>(self) -> Result<Tagged<NEW_TAG>, Self> {
        let mut arc = unsafe { ptr::read(&ManuallyDrop::new(self).0) };
        match Arc::get_mut(&mut arc) {
            Some(value) => {
                *value = NEW_TAG;
                Ok(Tagged(arc))
            }
            None => Err(Self(arc)),
        }
    }
}
impl<const TAG: usize> Drop for Tagged<TAG> {
    fn drop(&mut self) {
        assert_eq!(*self.0, TAG, "dropped with the wrong tag");
    }
}
unsafe impl<const TAG: usize> ArcPtr for Tagged<TAG> {
    unsafe fn from_ptr(ptr: *mut ()) -> Self {
        Self(unsafe { Arc::from_raw(ptr.cast()) })
    }
    fn into_ptr(arc: Self) -> *mut () {
        let arc = unsafe { ptr::read(&ManuallyDrop::new(arc).0) };
        Arc::into_raw(arc).cast_mut().cast()
    }
    fn as_ptr(arc: &Self) -> *mut () {
        Arc::as_ptr(&arc.0).cast_mut().cast()
    }
}

/// An unvalidated borrow (the pointer has changed after slot write) can be upgraded by another
/// `AtomicArc` reusing the same Arc (same address, not necessarily the same type). In that case,
/// this borrow should not be used, and the upgrade should be dropped with the appropriate
/// destructor.
///
/// This is obvious as written, but a previous version didn't consider such interleaving, and
/// assumed the borrow was upgraded by the same `AtomicArc` and was thus valid.
#[rstest]
fn unvalidated_transferred_borrow<D: Domain, W: WritePolicy>(
    #[values(d!(0), d!(1), d!(8), dm!(0), dm!(1), dm!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    model::<D>(|| {
        let a = AtomicArcPtr::<Tagged<0>, D, W>::new(Tagged::new());
        let b = AtomicArcPtr::<Tagged<1>, D, W>::new(Tagged::new());
        thread::scope(|s| {
            s.spawn(|| {
                if let Ok(arc) = a.swap(Tagged::new()).try_retag() {
                    b.store(arc);
                    b.store(Tagged::new());
                }
            });
            s.spawn(|| drop(a.load()));
        });
    });
}

/// The domain GC, triggered by the release of the reader's node at the end of its thread, must
/// not deallocate the nodes while the writer traverses them.
#[cfg(feature = "domain-gc")]
#[rstest]
fn gc_concurrent_write<D: Domain>(#[values(d!(1), dm!(1))] _domain: D) {
    model::<D>(|| {
        let atomic_arc = AtomicArc::<usize, D, Concurrent>::new(Arc::new(0));
        thread::scope(|s| {
            s.spawn(|| drop(atomic_arc.load()));
            s.spawn(|| atomic_arc.store(Arc::new(1)));
        });
    });
}

/// Creates an isolated domain with one borrow slot, whose `static_list` panics on its second call,
/// i.e. at the beginning of the first write scan following the first load.
#[cfg(not(loom))]
macro_rules! panicking_domain {
    () => {{
        use std::{
            cell::Cell,
            sync::atomic::{AtomicUsize, Ordering::Relaxed},
        };

        use hazarc::domain::{DomainList, DomainNodeRef};

        struct PanickingDomain;
        thread_local! {
            static LOCAL: Cell<Option<DomainNodeRef<PanickingDomain>>> = const { Cell::new(None) };
        }
        unsafe impl Domain for PanickingDomain {
            const BORROW_SLOT_COUNT: usize = 1;
            fn static_list() -> &'static DomainList<Self> {
                static CALLS: AtomicUsize = AtomicUsize::new(0);
                static LIST: DomainList<PanickingDomain> = DomainList::new();
                if CALLS.fetch_add(1, Relaxed) == 1 {
                    panic!("static list access");
                }
                &LIST
            }
            fn get_thread_local_node() -> Option<DomainNodeRef<Self>> {
                LOCAL.with(Cell::get)
            }
            unsafe fn set_thread_local_node(node: Option<DomainNodeRef<Self>>) {
                LOCAL.with(|local| local.set(node));
            }
        }
        PanickingDomain
    }};
}

/// If a write panics during the scan, the old Arc must not be dropped, as borrows may not have
/// been upgraded yet.
#[cfg(not(loom))]
#[rstest]
fn panic_during_write_scan<D: Domain, W: WritePolicy>(
    #[values(panicking_domain!())] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    use std::panic;
    let atomic_arc = AtomicArc::<usize, D, W>::new(Arc::new(0));
    let borrow = atomic_arc.load();
    let store = || atomic_arc.store(Arc::new(1));
    assert!(panic::catch_unwind(panic::AssertUnwindSafe(store)).is_err());
    assert_eq!(**borrow, 0);
    // The old Arc is leaked by the panicking store
    let old_ptr = Arc::as_ptr(&borrow);
    drop(borrow);
    unsafe { Arc::decrement_strong_count(old_ptr) };
}

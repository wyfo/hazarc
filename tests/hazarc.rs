use std::{
    cell::Cell,
    mem::ManuallyDrop,
    panic, ptr,
    sync::{
        atomic::{AtomicUsize, Ordering::Relaxed},
        Arc,
    },
    thread,
};

use hazarc::{
    arc::ArcPtr,
    atomic::AtomicArcPtr,
    domain::{Domain, DomainList, DomainNodeRef},
    write_policy::{Concurrent, Serialized, WritePolicy},
    ArcBorrow, AtomicArc, AtomicOptionArc,
};
use rstest::rstest;

/// Creates isolated domains per tests
macro_rules! test_domain {
    ($slots:literal) => {{
        hazarc::domain!(TestDomain($slots));
        TestDomain
    }};
}

#[rstest]
fn concurrent_reads<D: Domain, W: WritePolicy>(
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    let check_borrow = |b: &_| assert!([0, 1, 2].contains(b));
    let atomic_arc = AtomicArc::<usize, D, W>::from(0);
    thread::scope(|s| {
        s.spawn(|| check_borrow(&atomic_arc.load()));
        s.spawn(|| check_borrow(&atomic_arc.load()));
        s.spawn(|| {
            atomic_arc.store(1.into());
            atomic_arc.store(2.into());
        });
    });
}

#[rstest]
fn concurrent_reads_other_atomic_arc<D: Domain, W: WritePolicy>(
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    let check_borrow = |b: &_| assert!([0, 1].contains(b));
    let atomic_arc = AtomicArc::<usize, D, W>::from(0);
    let other = AtomicArc::<usize, D, W>::from(42);
    thread::scope(|s| {
        s.spawn(|| {
            let borrow1 = atomic_arc.load();
            let other_borrow = other.load();
            let borrow2 = atomic_arc.load();
            check_borrow(&borrow1);
            assert_eq!(**other_borrow, 42);
            check_borrow(&borrow2);
        });
        s.spawn(|| atomic_arc.store(1.into()));
    });
}

#[rstest]
fn concurrent_writes<D: Domain, W: WritePolicy>(
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    let check_borrow = |b: &_| assert!([0, 1, 2].contains(b));
    let atomic_arc = AtomicArc::<usize, D, W>::from(0);
    thread::scope(|s| {
        s.spawn(|| {
            let swapped = atomic_arc.swap(1.into());
            assert!(*swapped == 0 || *swapped == 2);
        });
        s.spawn(|| {
            let swapped = atomic_arc.swap(2.into());
            assert!(*swapped == 0 || *swapped == 1);
        });
        s.spawn(|| {
            let guard = atomic_arc.load();
            check_borrow(&guard);
            check_borrow(&atomic_arc.load());
            check_borrow(&atomic_arc.load());
        });
    });
}

#[rstest]
fn concurrent_writes_option<D: Domain, W: WritePolicy>(
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    let check_borrow = |b: &Option<ArcBorrow<_>>| {
        assert!([Some(0), Some(1), None].contains(&b.as_ref().map(|b| ***b)));
    };
    let atomic_arc = AtomicOptionArc::<usize, D, W>::from(0);
    thread::scope(|s| {
        s.spawn(|| {
            let swapped = atomic_arc.swap(Some(1.into()));
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
}

#[rstest]
fn drop_atomic_arc_with_active_borrow<D: Domain, W: WritePolicy>(
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    let atomic_arc = AtomicArc::<usize, D, W>::from(0);
    let borrow = atomic_arc.load();
    drop(atomic_arc);
    drop(borrow);
}

#[rstest]
fn drop_borrow_in_another_thread<D: Domain, W: WritePolicy>(
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    let atomic_arc = AtomicOptionArc::<usize, D, W>::from(0);
    thread::scope(|s| {
        let borrow = s.spawn(|| atomic_arc.load());
        s.spawn(|| atomic_arc.store(None));
        drop(borrow.join().unwrap());
    });
}

#[rstest]
fn seq_cst_ordering<D: Domain, W: WritePolicy>(
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    let x = AtomicOptionArc::<(), D, W>::new(None);
    let y = AtomicOptionArc::<(), D, W>::new(None);
    thread::scope(|s| {
        let a = s.spawn(|| {
            x.store(Some(().into()));
            y.load()
        });
        let b = s.spawn(|| {
            y.store(Some(().into()));
            x.load()
        });
        assert!(a.join().unwrap().is_some() || b.join().unwrap().is_some());
    });
}

#[rstest]
fn fetch_and_add<D: Domain>(
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
) {
    let atomic_arc = AtomicArc::<usize, D, Concurrent>::from(0);
    thread::scope(|s| {
        s.spawn(|| atomic_arc.update(|i| **i + 1));
        s.spawn(|| atomic_arc.update(|i| **i + 1));
    });
    assert_eq!(**atomic_arc.load(), 2);
}

/// Consecutive loads must be monotonic with `Concurrent`.
#[rstest]
fn consecutive_loads<D: Domain>(
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
) {
    let atomic_arc = AtomicArc::<usize, D, Concurrent>::from(0);
    thread::scope(|s| {
        s.spawn(|| atomic_arc.store(1.into()));
        s.spawn(|| atomic_arc.store(2.into()));
        s.spawn(|| {
            let a1 = atomic_arc.load();
            let a2 = atomic_arc.load();
            if **a1 != **a2 && **a1 != 0 {
                assert_eq!(**a2, **atomic_arc.load());
            }
        });
    });
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
    #[values(test_domain!(0), test_domain!(1), test_domain!(8))] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
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
}

/// Creates an isolated domain with one borrow slot, whose `static_list` panics on its second call,
/// i.e. at the beginning of the first write scan following the first load.
macro_rules! panicking_domain {
    () => {{
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
#[rstest]
fn panic_during_write_scan<D: Domain, W: WritePolicy>(
    #[values(panicking_domain!())] _domain: D,
    #[values(Concurrent, Serialized)] _policy: W,
) {
    let atomic_arc = AtomicArc::<usize, D, W>::from(0);
    let borrow = atomic_arc.load();
    let store = || atomic_arc.store(1.into());
    assert!(panic::catch_unwind(panic::AssertUnwindSafe(store)).is_err());
    assert_eq!(**borrow, 0);
    // The old Arc is leaked by the panicking store
    let old_ptr = Arc::as_ptr(&borrow);
    drop(borrow);
    unsafe { Arc::decrement_strong_count(old_ptr) };
}

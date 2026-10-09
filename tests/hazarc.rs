use std::{sync::Arc, thread};

use hazarc::{
    domain::Domain,
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

use std::{
    any::TypeId,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
    thread,
};

use hazarc::{ArcBorrow, AtomicArc, AtomicOptionArc, domain};

use super::LoadPolicy;

struct SpinBarrier(AtomicUsize);

impl SpinBarrier {
    fn new(n: usize) -> Self {
        Self(AtomicUsize::new(n))
    }

    fn wait(&self) {
        self.0.fetch_sub(1, Relaxed);
        while self.0.load(Relaxed) != 0 {}
    }

    fn wrap<R: Send>(&self, f: impl FnOnce() -> R + Send) -> impl FnOnce() -> R + Send {
        || {
            self.wait();
            f()
        }
    }
}

#[test]
fn concurrent_writes() {
    domain!(TestDomain(1));
    let check_borrow = |b: &ArcBorrow<_>| assert!([0, 1, 2].contains(b));
    let barrier = SpinBarrier::new(3);
    let atomic_arc = AtomicArc::<_, TestDomain, LoadPolicy>::from(0);
    thread::scope(|s| {
        s.spawn(barrier.wrap(|| {
            let swapped = atomic_arc.swap(1.into());
            assert!(*swapped == 0 || *swapped == 2)
        }));
        s.spawn(barrier.wrap(|| {
            let swapped = atomic_arc.swap(2.into());
            assert!(*swapped == 0 || *swapped == 1)
        }));
        barrier.wait();
        let guard = atomic_arc.load();
        check_borrow(&guard);
        check_borrow(&atomic_arc.load());
        check_borrow(&atomic_arc.load());
    });
}

#[test]
fn concurrent_writes_option() {
    domain!(TestDomain(1));
    let check_borrow = |b: &Option<ArcBorrow<_>>| {
        assert!([Some(0), Some(1), None].contains(&b.as_ref().map(|b| ***b)))
    };
    let barrier = SpinBarrier::new(3);
    let atomic_arc = AtomicOptionArc::<usize, TestDomain, LoadPolicy>::from(0);
    thread::scope(|s| {
        s.spawn(barrier.wrap(|| {
            let swapped = atomic_arc.swap(Some(1.into()));
            assert!(swapped.as_deref() == Some(&0) || swapped.is_none())
        }));
        s.spawn(barrier.wrap(|| {
            let swapped = atomic_arc.swap(None);
            assert!(swapped.as_deref() == Some(&1) || swapped.as_deref() == Some(&0))
        }));
        barrier.wait();
        let guard = atomic_arc.load();
        check_borrow(&guard);
        check_borrow(&atomic_arc.load());
        check_borrow(&atomic_arc.load());
    });
}

#[test]
fn drop_atomic_arc_with_active_borrow() {
    domain!(TestDomain(1));
    let atomic_arc = AtomicArc::<usize, TestDomain, LoadPolicy>::from(0);
    let borrow = atomic_arc.load();
    drop(atomic_arc);
    drop(borrow);
}

#[test]
fn drop_borrow_in_another_thread() {
    domain!(TestDomain(1));
    let barrier = SpinBarrier::new(2);
    let atomic_arc = AtomicOptionArc::<usize, TestDomain, LoadPolicy>::from(0);
    thread::scope(|s| {
        let thread = s.spawn(barrier.wrap(|| atomic_arc.load()));
        barrier.wait();
        atomic_arc.store(None);
        let borrow = thread.join().unwrap();
        drop(borrow);
    });
}

#[test]
fn fetch_and_add() {
    domain!(TestDomain(1));
    let barrier = SpinBarrier::new(2);
    let atomic_arc = AtomicArc::<usize, TestDomain, LoadPolicy>::from(0);
    thread::scope(|s| {
        s.spawn(barrier.wrap(|| atomic_arc.fetch_update(|i| Some(**i + 1))));
        s.spawn(barrier.wrap(|| atomic_arc.fetch_update(|i| Some(**i + 1))));
    });
    assert_eq!(**atomic_arc.load(), 2);
}

#[test]
fn consecutive_loads() {
    if TypeId::of::<LoadPolicy>() == TypeId::of::<hazarc::load_policy::WaitFree>() {
        return;
    }
    domain!(TestDomain(1));
    let atomic_arc = AtomicArc::<usize, TestDomain, LoadPolicy>::from(0);
    let barrier = SpinBarrier::new(3);
    thread::scope(|s| {
        s.spawn(barrier.wrap(|| atomic_arc.store(1.into())));
        s.spawn(barrier.wrap(|| atomic_arc.store(2.into())));
        barrier.wait();
        let a1 = atomic_arc.load();
        let a2 = atomic_arc.load();
        if **a1 != **a2 && **a1 != 0 {
            assert_eq!(**a2, **atomic_arc.load());
        }
    });
}

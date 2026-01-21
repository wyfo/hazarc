use std::{
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
    thread,
};

use hazarc::{ArcBorrow, AtomicArc, AtomicOptionArc, borrow_list};

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
    borrow_list!(TestList(1));
    let check_borrow = |b: &ArcBorrow<_>| assert!([0, 1, 2].contains(b));
    let barrier = SpinBarrier::new(3);
    let atomic_arc = AtomicArc::<_, TestList>::from(0);
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
    borrow_list!(TestList(1));
    let check_borrow = |b: &Option<ArcBorrow<_>>| {
        assert!([Some(0), Some(1), None].contains(&b.as_ref().map(|b| ***b)))
    };
    let barrier = SpinBarrier::new(3);
    let atomic_arc = AtomicOptionArc::<usize, TestList>::from(0);
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
    borrow_list!(TestList(1));
    let atomic_arc = AtomicArc::<usize, TestList>::from(0);
    let borrow = atomic_arc.load();
    drop(atomic_arc);
    drop(borrow);
}

#[test]
fn drop_borrow_in_another_thread() {
    borrow_list!(TestList(1));
    let barrier = SpinBarrier::new(2);
    let atomic_arc = AtomicOptionArc::<usize, TestList>::from(0);
    thread::scope(|s| {
        let thread = s.spawn(barrier.wrap(|| atomic_arc.load()));
        barrier.wait();
        atomic_arc.store(None);
        let borrow = thread.join().unwrap();
        drop(borrow);
    });
}

use std::{
    mem,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
    thread,
};

use hazarc::{
    ArcBorrow, AtomicArc, AtomicOptionArc, domain,
    domain::{BorrowNodeRef, Domain},
};

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
    let atomic_arc = AtomicArc::<_, TestDomain>::from(0);
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
    let atomic_arc = AtomicOptionArc::<usize, TestDomain>::from(0);
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
    let atomic_arc = AtomicArc::<usize, TestDomain>::from(0);
    let borrow = atomic_arc.load();
    drop(atomic_arc);
    drop(borrow);
}

#[test]
fn drop_borrow_in_another_thread() {
    domain!(TestDomain(1));
    let barrier = SpinBarrier::new(2);
    let atomic_arc = AtomicOptionArc::<usize, TestDomain>::from(0);
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
    let atomic_arc = AtomicArc::<usize, TestDomain>::from(0);
    thread::scope(|s| {
        s.spawn(barrier.wrap(|| atomic_arc.fetch_update(|i| Some(**i + 1))));
        s.spawn(barrier.wrap(|| atomic_arc.fetch_update(|i| Some(**i + 1))));
    });
    assert_eq!(**atomic_arc.load(), 2);
}

#[test]
fn borrow_list() {
    domain!(TestDomain(1));
    let barrier = SpinBarrier::new(2);
    thread::scope(|s| {
        let node1 = s.spawn(TestDomain::thread_local_node).join().unwrap();
        let thread2 = s.spawn(barrier.wrap(TestDomain::thread_local_node));
        let thread3 = s.spawn(barrier.wrap(TestDomain::thread_local_node));
        let node2 = thread2.join().unwrap();
        let node3 = thread3.join().unwrap();
        let ptr = |n| unsafe { mem::transmute::<BorrowNodeRef, *mut ()>(n) };
        assert!(ptr(node1) == ptr(node2) || ptr(node1) == ptr(node3));
    });
}

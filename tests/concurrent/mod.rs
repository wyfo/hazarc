use std::thread;

use hazarc::{domain, AtomicArc};

use super::{common::SpinBarrier, WritePolicy, SLOTS};

#[test]
fn fetch_and_add() {
    domain!(TestDomain(SLOTS));
    let barrier = SpinBarrier::new(2);
    let atomic_arc = AtomicArc::<usize, TestDomain, WritePolicy>::from(0);
    thread::scope(|s| {
        s.spawn(barrier.wrap(|| atomic_arc.fetch_update(|i| Some(**i + 1))));
        s.spawn(barrier.wrap(|| atomic_arc.fetch_update(|i| Some(**i + 1))));
    });
    assert_eq!(**atomic_arc.load(), 2);
}

#[test]
fn consecutive_loads() {
    domain!(TestDomain(SLOTS));
    let atomic_arc = AtomicArc::<usize, TestDomain, WritePolicy>::from(0);
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

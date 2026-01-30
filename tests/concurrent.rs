use std::thread;

#[allow(unused_imports)]
use hazarc::write_policy::Concurrent as WritePolicy;
use hazarc::{domain, AtomicArc};

use crate::concurrent::SpinBarrier;

#[path = "common/mod.rs"]
mod concurrent;

#[test]
fn fetch_and_add() {
    domain!(TestDomain(1));
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
    domain!(TestDomain(1));
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

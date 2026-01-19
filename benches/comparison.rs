use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
    thread,
    time::Duration,
};

use arc_swap::{ArcSwap, ArcSwapOption};
use divan::Bencher;
use hazarc::{AtomicArc, AtomicOptionArc};

#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn arcswap_write(b: Bencher, thread_count: usize) {
    let v: Arc<usize> = 0.into();
    let arc: Arc<ArcSwap<usize>> = Arc::new(ArcSwap::new(v.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    for _ in 0..thread_count {
        let arc = arc.clone();
        let stop = stop.clone();
        threads.push(thread::spawn(move || {
            while !stop.load(Relaxed) {
                arc.load();
                for _ in 0..32 {
                    std::hint::spin_loop();
                }
            }
        }));
    }
    thread::sleep(Duration::from_secs(1));
    b.bench(|| arc.swap(v.clone()));
    stop.store(true, Relaxed);
    for thread in threads {
        thread.join().unwrap();
    }
}

#[divan::bench(threads = [1, 2, 4, 8, 16], args = [false, true])]
fn arcswap_read(b: Bencher, write: bool) {
    let v: Arc<usize> = 0.into();
    let arc: Arc<ArcSwap<usize>> = Arc::new(ArcSwap::new(v.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let v = v.clone();
        let arc = arc.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            while !stop.load(Relaxed) {
                if write {
                    arc.store(v.clone());
                }
                for _ in 0..256 {
                    std::hint::spin_loop();
                }
            }
        })
    };
    thread::sleep(Duration::from_secs(1));
    b.with_inputs(|| drop(arc.load()))
        .bench_values(|()| drop(arc.load()));
    stop.store(true, Relaxed);
    thread.join().unwrap();
}

#[divan::bench]
fn arcswap_read_none(b: Bencher) {
    let atomic_arc = ArcSwapOption::<usize>::empty();
    b.with_inputs(|| drop(atomic_arc.load()))
        .bench_values(|()| drop(atomic_arc.load()));
}

#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn rwlock_write(b: Bencher, thread_count: usize) {
    let v = Arc::new(0);
    let lock = Arc::new(RwLock::new(v.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    for _ in 0..thread_count {
        let lock = lock.clone();
        let stop = stop.clone();
        threads.push(thread::spawn(move || {
            while !stop.load(Relaxed) {
                let clone = lock.read().unwrap().clone();
                drop(clone);
                for _ in 0..32 {
                    std::hint::spin_loop();
                }
            }
        }));
    }
    thread::sleep(Duration::from_secs(1));
    b.bench(|| *lock.write().unwrap() = v.clone());
    stop.store(true, Relaxed);
    for thread in threads {
        thread.join().unwrap();
    }
}

#[divan::bench(threads = [1, 2, 4, 8, 16], args = [false, false, true])]
fn rwlock_read(b: Bencher, write: bool) {
    let v: Arc<usize> = 0.into();
    let lock = Arc::new(RwLock::new(v.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let v = v.clone();
        let lock = lock.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            while !stop.load(Relaxed) {
                if write {
                    *lock.write().unwrap() = v.clone();
                }
                for _ in 0..256 {
                    std::hint::spin_loop();
                }
            }
        })
    };
    thread::sleep(Duration::from_secs(1));
    b.bench(|| {
        let clone = lock.read().unwrap().clone();
        drop(clone);
    });
    stop.store(true, Relaxed);
    thread.join().unwrap();
}

#[divan::bench(threads = [1, 2, 4, 8, 16], args = [false, true])]
fn rwlock_read_no_clone(b: Bencher, write: bool) {
    let v: Arc<usize> = 0.into();
    let lock = Arc::new(RwLock::new(v.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let v = v.clone();
        let lock = lock.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            while !stop.load(Relaxed) {
                if write {
                    *lock.write().unwrap() = v.clone();
                }
                for _ in 0..256 {
                    std::hint::spin_loop();
                }
            }
        })
    };
    thread::sleep(Duration::from_secs(1));
    b.bench(|| {
        let _lock = lock.read().unwrap();
    });
    stop.store(true, Relaxed);
    thread.join().unwrap();
}

#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn hazarc_write(b: Bencher, thread_count: usize) {
    let v: Arc<usize> = 0.into();
    let arc: Arc<AtomicArc<usize>> = Arc::new(AtomicArc::new(v.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    for _ in 0..thread_count {
        let arc = arc.clone();
        let stop = stop.clone();
        threads.push(thread::spawn(move || {
            while !stop.load(Relaxed) {
                arc.load();
                for _ in 0..32 {
                    std::hint::spin_loop();
                }
            }
        }));
    }
    thread::sleep(Duration::from_secs(1));
    b.bench(|| arc.swap(v.clone()));
    stop.store(true, Relaxed);
    for thread in threads {
        thread.join().unwrap();
    }
}

#[divan::bench(threads = [1, 2, 4, 8, 16], args = [false, true])]
fn hazarc_read(b: Bencher, write: bool) {
    let v: Arc<usize> = 0.into();
    let arc: Arc<AtomicArc<usize>> = Arc::new(AtomicArc::new(v.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let v = v.clone();
        let arc = arc.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            while !stop.load(Relaxed) {
                if write {
                    arc.store(v.clone());
                }
                for _ in 0..256 {
                    std::hint::spin_loop();
                }
            }
        })
    };
    thread::sleep(Duration::from_secs(1));
    b.with_inputs(|| drop(arc.load()))
        .bench_values(|()| drop(arc.load()));
    stop.store(true, Relaxed);
    thread.join().unwrap();
}

#[divan::bench]
fn hazarc_read_none(b: Bencher) {
    let atomic_arc = AtomicOptionArc::<usize>::none();
    b.with_inputs(|| drop(atomic_arc.load()))
        .bench_values(|()| drop(atomic_arc.load()));
}

fn main() {
    divan::main()
}

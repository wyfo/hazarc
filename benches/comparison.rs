use std::{
    hint,
    hint::black_box,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
        Arc, Barrier, RwLock, RwLockReadGuard,
    },
    thread,
};

use arc_swap::{ArcSwap, ArcSwapOption, Guard};
use divan::Bencher;
use hazarc::{domain::Domain, ArcBorrow, AtomicArc, AtomicOptionArc, DefaultDomain};

trait LoadBench: Default + Send + Sync {
    const SLOT_COUNT: usize = 0;
    type Guard<'a>
    where
        Self: 'a;
    fn load(&self) -> Self::Guard<'_>;
    fn bench_load(b: Bencher, threads: bool) {
        let x = black_box(Self::default());
        drop(x.load());
        if threads {
            b.bench(|| drop(x.load()));
        } else {
            b.bench_local(|| drop(x.load()));
        }
    }
    fn bench_load_no_slot(b: Bencher) {
        let x = black_box(Self::default());
        let _guards = (0..Self::SLOT_COUNT).map(|_| x.load()).collect::<Vec<_>>();
        b.bench_local(|| drop(x.load()));
    }
}
trait StoreBench: LoadBench + From<Arc<usize>> {
    fn store(&self, arc: Arc<usize>);
    fn bench_load_contended(b: Bencher, threads: bool) {
        let arc = Arc::new(0);
        let x = black_box(Self::from(arc.clone()));
        drop(x.load());
        let started = AtomicBool::new(false);
        let stop = AtomicBool::new(false);
        thread::scope(|s| {
            s.spawn(|| {
                started.store(true, Relaxed);
                while !stop.load(Relaxed) {
                    x.store(arc.clone());
                    for _ in 0..8 {
                        hint::spin_loop();
                    }
                }
            });
            while !started.load(Relaxed) {
                hint::spin_loop();
            }
            let load = || drop(x.load());
            if threads {
                b.bench(load);
            } else {
                b.bench_local(load);
            }
            stop.store(true, Relaxed);
        });
    }

    fn bench_store(b: Bencher, threads: usize) {
        let arc = Arc::new(0);
        let x = black_box(Self::from(arc.clone()));
        let barrier = Barrier::new(threads);
        thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    drop(x.load());
                    barrier.wait();
                });
            }
        });
        b.bench_local(|| x.store(arc.clone()));
    }

    fn bench_store_contended(b: Bencher, threads: usize) {
        let arc = Arc::new(0);
        let atomic_arc = black_box(Self::from(arc.clone()));
        let started = AtomicUsize::new(0);
        let stop = AtomicBool::new(false);
        thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    started.fetch_add(1, Relaxed);
                    while !stop.load(Relaxed) {
                        let _guard = atomic_arc.load();
                        for _ in 0..8 {
                            hint::spin_loop();
                        }
                    }
                });
            }
            while started.load(Relaxed) != threads {
                hint::spin_loop();
            }
            b.bench_local(|| atomic_arc.store(arc.clone()));
            stop.store(true, Relaxed);
        });
    }
}

impl LoadBench for ArcSwap<usize> {
    const SLOT_COUNT: usize = 8;
    type Guard<'a> = Guard<Arc<usize>>;
    fn load(&self) -> Self::Guard<'_> {
        self.load()
    }
}
impl StoreBench for ArcSwap<usize> {
    fn store(&self, arc: Arc<usize>) {
        self.store(arc);
    }
}
impl LoadBench for ArcSwapOption<usize> {
    const SLOT_COUNT: usize = 8;
    type Guard<'a> = Guard<Option<Arc<usize>>>;
    fn load(&self) -> Self::Guard<'_> {
        self.load()
    }
}

impl<D: Domain> LoadBench for AtomicArc<usize, D> {
    const SLOT_COUNT: usize = D::BORROW_SLOT_COUNT;
    type Guard<'a> = ArcBorrow<usize>;
    fn load(&self) -> Self::Guard<'_> {
        self.load()
    }
}
impl<D: Domain> StoreBench for AtomicArc<usize, D> {
    fn store(&self, arc: Arc<usize>) {
        self.store(arc);
    }
}
impl<D: Domain> LoadBench for AtomicOptionArc<usize, D> {
    const SLOT_COUNT: usize = D::BORROW_SLOT_COUNT;
    type Guard<'a> = Option<ArcBorrow<usize>>;
    fn load(&self) -> Self::Guard<'_> {
        self.load()
    }
}
#[cfg(all(feature = "pthread-domain", unix))]
hazarc::pthread_domain!(PthreadDomain(DefaultDomain::BORROW_SLOT_COUNT));
#[cfg(all(feature = "pthread-domain", unix))]
hazarc::pthread_domain! {
    UnsafePthreadDomain(DefaultDomain::BORROW_SLOT_COUNT);
    // SAFETY: `UnsafePthreadDomain::pthread_key_create` is called
    // before using the domain in `hazarc_load_pthread_unsafe`
    unsafe { Self::pthread_key_already_created() }
}

#[cfg(feature = "membarrier")]
hazarc::membarrier_domain!(MembarrierDomain(DefaultDomain::BORROW_SLOT_COUNT));

hazarc::domain!(Domain8Slots(8));

impl LoadBench for RwLock<Arc<usize>> {
    type Guard<'a> = RwLockReadGuard<'a, Arc<usize>>;
    fn load(&self) -> Self::Guard<'_> {
        self.read().unwrap()
    }
}
impl StoreBench for RwLock<Arc<usize>> {
    fn store(&self, arc: Arc<usize>) {
        *self.write().unwrap() = arc;
    }
}

#[derive(Default)]
struct RwLockClone<T>(RwLock<T>);
impl LoadBench for RwLockClone<Arc<usize>> {
    type Guard<'a> = Arc<usize>;
    fn load(&self) -> Self::Guard<'_> {
        self.0.read().unwrap().clone()
    }
}
impl From<Arc<usize>> for RwLockClone<Arc<usize>> {
    fn from(arc: Arc<usize>) -> Self {
        Self(RwLock::new(arc))
    }
}
impl StoreBench for RwLockClone<Arc<usize>> {
    fn store(&self, arc: Arc<usize>) {
        *self.0.write().unwrap() = arc;
    }
}

#[derive(Default)]
struct LoadSpin<T>(T);
impl<T: LoadBench> LoadBench for LoadSpin<T> {
    const SLOT_COUNT: usize = T::SLOT_COUNT;
    type Guard<'a>
        = T::Guard<'a>
    where
        Self: 'a;
    fn load(&self) -> Self::Guard<'_> {
        let guard = self.0.load();
        hint::spin_loop();
        guard
    }
}

#[divan::bench]
fn arcswap_load(b: Bencher) {
    ArcSwap::bench_load(b, false);
}
#[divan::bench]
fn arcswap_load_spin(b: Bencher) {
    LoadSpin::<ArcSwap<_>>::bench_load(b, false);
}
#[divan::bench]
fn arcswap_load_no_slot(b: Bencher) {
    ArcSwap::bench_load_no_slot(b);
}
#[divan::bench]
fn arcswap_load_no_slot_spin(b: Bencher) {
    LoadSpin::<ArcSwap<_>>::bench_load_no_slot(b);
}
#[divan::bench]
fn arcswap_load_none(b: Bencher) {
    ArcSwapOption::bench_load(b, false);
}
#[divan::bench]
fn arcswap_load_contended(b: Bencher) {
    ArcSwap::bench_load_contended(b, false);
}
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn arcswap_store(b: Bencher, threads: usize) {
    ArcSwap::bench_store(b, threads);
}
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn arcswap_store_contended(b: Bencher, threads: usize) {
    ArcSwap::bench_store_contended(b, threads);
}

#[divan::bench]
fn hazarc_load(b: Bencher) {
    AtomicArc::<_, DefaultDomain>::bench_load(b, false);
}
#[cfg(all(feature = "pthread-domain", unix))]
#[divan::bench]
fn hazarc_load_pthread(b: Bencher) {
    AtomicArc::<_, PthreadDomain>::bench_load(b, false);
}
#[cfg(all(feature = "pthread-domain", unix))]
#[divan::bench]
fn hazarc_load_pthread_unsafe(b: Bencher) {
    UnsafePthreadDomain::pthread_key_create();
    AtomicArc::<_, UnsafePthreadDomain>::bench_load(b, false);
}
#[cfg(feature = "membarrier")]
#[divan::bench]
fn hazarc_load_membarrier(b: Bencher) {
    AtomicArc::<_, MembarrierDomain>::bench_load(b, false);
}
#[divan::bench]
fn hazarc_load_spin(b: Bencher) {
    LoadSpin::<AtomicArc<_, DefaultDomain>>::bench_load(b, false);
}
#[divan::bench]
fn hazarc_load_no_slot(b: Bencher) {
    AtomicArc::<_, DefaultDomain>::bench_load_no_slot(b);
}
#[divan::bench]
fn hazarc_load_no_slot_spin(b: Bencher) {
    LoadSpin::<AtomicArc<_, DefaultDomain>>::bench_load_no_slot(b);
}
#[divan::bench]
fn hazarc_load_none(b: Bencher) {
    AtomicOptionArc::<_, DefaultDomain>::bench_load(b, false);
}
#[divan::bench]
fn hazarc_load_contended(b: Bencher) {
    AtomicArc::<_, DefaultDomain>::bench_load_contended(b, false);
}
#[cfg(feature = "membarrier")]
#[divan::bench]
fn hazarc_load_contended_membarrier(b: Bencher) {
    AtomicArc::<_, MembarrierDomain>::bench_load_contended(b, false);
}
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn hazarc_store(b: Bencher, threads: usize) {
    AtomicArc::<_, DefaultDomain>::bench_store(b, threads);
}
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn hazarc_store_8_slots(b: Bencher, threads: usize) {
    AtomicArc::<_, Domain8Slots>::bench_store(b, threads);
}
#[cfg(feature = "membarrier")]
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn hazarc_store_membarrier(b: Bencher, threads: usize) {
    AtomicArc::<_, MembarrierDomain>::bench_store(b, threads);
}
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn hazarc_store_contended(b: Bencher, threads: usize) {
    AtomicArc::<_, DefaultDomain>::bench_store_contended(b, threads);
}
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn hazarc_store_contended_8_slots(b: Bencher, threads: usize) {
    AtomicArc::<_, Domain8Slots>::bench_store_contended(b, threads);
}
#[cfg(feature = "membarrier")]
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn hazarc_store_contended_membarrier(b: Bencher, threads: usize) {
    AtomicArc::<_, MembarrierDomain>::bench_store_contended(b, threads);
}

#[divan::bench(threads = [0, 1, 2, 4, 8, 16])]
fn rwlock_read(b: Bencher) {
    RwLock::bench_load(b, true);
}
#[divan::bench(threads = [0, 1, 2, 4, 8, 16])]
fn rwlock_read_spin(b: Bencher) {
    LoadSpin::<RwLock<_>>::bench_load(b, true);
}
#[divan::bench(threads = [0, 1, 2, 4, 8, 16])]
fn rwlock_read_clone(b: Bencher) {
    RwLockClone::bench_load(b, true);
}
#[divan::bench(threads = [0, 1, 2, 4, 8, 16])]
fn rwlock_read_clone_spin(b: Bencher) {
    LoadSpin::<RwLockClone<_>>::bench_load(b, true);
}
#[divan::bench(threads = [0, 1, 2, 4, 8, 16])]
fn rwlock_read_contended(b: Bencher) {
    RwLock::bench_load_contended(b, true);
}
#[divan::bench(threads = [0, 1, 2, 4, 8, 16])]
fn rwlock_read_contended_clone(b: Bencher) {
    RwLockClone::bench_load_contended(b, true);
}
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn rwlock_write(b: Bencher, threads: usize) {
    RwLock::bench_store(b, threads);
}
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn rwlock_write_contended(b: Bencher, threads: usize) {
    RwLock::bench_store_contended(b, threads);
}
#[divan::bench(args = [0, 1, 2, 4, 8, 16])]
fn rwlock_write_contended_clone(b: Bencher, threads: usize) {
    RwLockClone::bench_store_contended(b, threads);
}

fn main() {
    divan::main();
}

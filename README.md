# hazarc

A wait-free `AtomicArc` optimized for read-intensive use cases.

## Acknowledgement

This library is based on the brilliant idea behind [`arc-swap`](https://github.com/vorner/arc-swap) from [Michal Vaner](https://github.com/vorner): mixing hazard pointer-based protection with atomic reference counting.

## Examples

```rust
use hazarc::AtomicArc;

struct Config { /* ... */ }

fn update_config(shared_cfg: &AtomicArc<Config>, /* ... */) {
    shared_cfg.store(/* ... */);
}

fn task(shared_cfg: &AtomicArc<Config>) {
    loop {
        let cfg = shared_cfg.load();
        /* ... */
    }
}
```

`AtomicArc::load` is already very fast, but `Cache::load` is blazingly fast.

```rust
use std::sync::Arc;
use hazarc::AtomicArc;

struct Config { /* ... */ }

fn update_config(shared_cfg: &AtomicArc<Config>, /* ... */) {
    shared_cfg.store(/* ... */);
}

fn spawn_task(shared_cfg: Arc<AtomicArc<Config>>) {
    thread::spawn(move || {
        let mut cache = hazarc::Cache::new(shared_cfg);
        loop {
            let cfg = cache.load();
            /* ... */
        }
    });
}
```

With custom domains, it can be used in a `no_std` environment.

```rust
#![no_std]
extern crate alloc;

use alloc::sync::Arc;
use hazarc::AtomicArc;

hazarc::pthread_domain!(NoStdDomain(2)); // 2 hazard pointer slots
struct Config { /* ... */ }

fn update_config(shared_cfg: &AtomicArc<Config, NoStdDomain>, /* ... */) {
    shared_cfg.store(/* ... */);
}

fn task(shared_cfg: &AtomicArc<Config, NoStdDomain>) {
    loop {
        let cfg = shared_cfg.load();
        /* ... */
    }
}
```

## Wait-freedom

### Load

#### Thread-local storage

`AtomicArc::load` relies on a domain's thread-local node which is lazily allocated and inserted in the domain's global list on first access. As a consequence, the first `AtomicArc::load` for a given domain may not be wait-free.

It is however possible to access the thread-local node explicitly before using `AtomicArc`, making all subsequent accesses wait-free. Another solution is to pre-allocate the number of nodes required by the program. In that case, insertion into the domain's global list is bounded by the number of allocated nodes, and thread-local accesses are wait-free.

#### Load policy

The rest of `AtomicArc::load` algorithm is determined by a generic `LoadPolicy` with the following variants:
- `WaitFree`, loads are wait-free, but may cause non-monotonic reads as soon as there are **multiple concurrent stores**. When stores are serialized — with a lock, a MPSC task, etc. —, loads are guaranteed to be monotonic. *Multiple loads concurrent with a single store is not an issue.*  
- `LockFree`, loads are lock-free and supports multiple concurrent stores.
- `Adaptive` (the default), loads are wait-free until multiple concurrent stores happen. At that point, loads are downgraded to lock-free for the given `AtomicArc` until it is dropped. The impact on performance is [mostly negligible](benches/README.md) compared to `WaitFree`.

### Store

`AtomicArc::store`, which wraps `AtomicArc::swap`, needs to scan the whole domain's global list, executing a bounded number of atomic operations on each node. If the number of nodes is bounded as well — which should be the case most of the time — then the whole operation is wait-free.


## Safety

This library uses unsafe code to deal with `AtomicPtr` manipulation and DST allocations. It is extensively tested with [`miri`](https://github.com/rust-lang/miri) to ensure its soundness, including over multiple weak memory model permutations.
 
## Differences with `arc-swap`

- Custom domains to reduce contention and add `no_std` support
- Enforce monotonic reads — a thread cannot observe an older value after having observed a newer one
- Wait-free `AtomicArc::swap` — `ArcSwap::swap` is only lock-free
- Less atomic RMW instructions
- `AtomicArc::load` critical path fully inlined
- [Better performance](benches/README.md), especially on ARM architecture
- Null pointer/`None` load optimized
- Ergonomic API for `Option`, `AtomicOptionArc<T>::load` returns `Option<ArcBorrow<T>>`
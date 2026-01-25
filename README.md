# hazarc

A wait-free `AtomicArc` optimized for read-intensive use cases.

## Acknowledgement

This library is based[^1] on the brilliant idea of [`arc-swap`](https://github.com/vorner/arc-swap) from [Michal Vaner](https://github.com/vorner): mixing hazard pointer-based protection with atomic reference counting.

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
 
## Differences with `arc-swap`

- Custom domains to reduce contention and add `no_std` support
- Wait-free `AtomicArc::swap` — `ArcSwap::swap` is only lock-free
- Less atomic RMW instructions
- Better performance, especially on ARM architecture
- `AtomicArc::load` critical path inlined in less than 30 assembly instructions
- Null pointer/`None` load optimized
- Ergonomic API for `Option`, `AtomicOptionArc<T>::load` returns `Option<ArcBorrow<T>>`

## Safety

This library uses unsafe code to deal with `AtomicPtr` manipulation and DST allocations. It is extensively tested with [`miri`](https://github.com/rust-lang/miri) to ensure its soundness, including over multiple weak memory model permutations.

[^1]: The idea is the same: try to acquire a hazard-pointer-like slot, falling back to acquiring a full ownership of the loaded arc in a wait-free manner. The underlying algorithm is different, both for the hazard pointer part and especially for the fallback part — it allows `AtomicArc` to be fully wait-free.
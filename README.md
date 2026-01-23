# hazarc

A wait-free `AtomicArc` optimized for read-intensive use cases.

## Acknowledgement

This library is based on the genius idea of `arc-swap` from [Michal Vaner](https://github.com/vorner): mixing hazard pointers with `Arc`s, with a fallback algorithm to acquire full ownership in case borrowing failure.

## Examples

```rust
use hazarc::AtomicArc;

struct Config;

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

`AtomicArc::load` is already very fast, but `ArcCache::load` is blazingly fast

```rust
use std::sync::Arc;
use hazarc::{AtomicArc, cache::ArcCache};

struct Config;

fn update_config(shared_cfg: &AtomicArc<Config>, /* ... */) {
    shared_cfg.store(/* ... */);
}

fn spawn_task(shared_cfg: Arc<AtomicArc<Config>>) {
    thread::spawn(move || {
        let mut cache = ArcCache::new(shared_cfg);
        loop {
            let cfg = cache.load();
            /* ... */
        }
    });
}
```
 
## Differences with `arc-swap`

- Custom domains to reduce contention
- Wait-free `AtomicArc::swap` thanks to an original load fallback algorithm
- `AtomicArc::load` critical path inlined in less than 30 instructions
- Less atomic RMW instructions
- Better performances, especially on ARM architecture
- Ergonomic API for `Option`, `AtomicOptionArc<T>::load` returns `Option<ArcBorrow<T>>`
- Null pointer/`None` load optimized
- `AtomicArc` is a more intuitive name than `ArcSwap`

//! Cache wrapper to optimize atomic loads of `Arc`-like pointers.

use crate::{
    arc::NonNullArcPtr,
    atomic::{AtomicArcRef, CachedOrReloaded},
};

/// A cache for a shared [`AtomicArc`](crate::atomic::AtomicArcPtr).
///
/// Built as a wrapper around [`AtomicArc::load_cached`](crate::atomic::AtomicArcPtr::load_cached),
/// it essentially makes loads of up-to-date Arcs free, but requires a mutable reference.
///
/// As the cache stores the latest loaded Arc, it can delay its reclamation until a new Arc
/// is loaded.
///
/// # Examples
///
/// ```rust
/// # use std::sync::Arc;
/// # hazarc::domain!(Domain);
/// # type AtomicArc<T> = hazarc::AtomicArc<T, Domain>;
/// let atomic_arc = Arc::new(AtomicArc::<usize>::from(0));
/// let mut cache = hazarc::Cache::new(atomic_arc);
/// assert_eq!(**cache.load(), 0);
/// ```
///
/// It also works with `Option`.
///
/// ```rust
/// # use std::sync::Arc;
/// # hazarc::domain!(Domain);
/// # type AtomicArcPtr<A> = hazarc::atomic::AtomicArcPtr<A, Domain, hazarc::write_policy::Concurrent>;
/// let atomic_arc = Arc::new(AtomicArcPtr::<Option<Arc<usize>>>::none());
/// let mut cache = hazarc::Cache::new(atomic_arc);
/// assert_eq!(cache.load_opt(), None);
/// ```
#[derive(Debug, Clone)]
pub struct Cache<A: AtomicArcRef> {
    inner: A,
    cached: A::Arc,
}

impl<A: AtomicArcRef> Cache<A> {
    /// Constructs a new `Cache`, loading and storing the up-to-date Arc.
    #[inline]
    pub fn new(inner: A) -> Self {
        let cached = inner.atomic_arc().load_owned();
        Self { inner, cached }
    }

    /// Accesses the inner shared `AtomicArc`.
    pub fn inner(&self) -> &A {
        &self.inner
    }

    /// Consumes the cache to returns the inner shared `AtomicArc`.
    pub fn into_inner(self) -> A {
        self.inner
    }

    /// Returns a reference the cached Arc, updating it when it is outdated.
    #[inline]
    pub fn load(&mut self) -> &A::Arc {
        self.inner.atomic_arc().load_cached(&mut self.cached)
    }

    /// Returns a reference to the cached Arc if it is up-to-date, or loads the latest Arc.
    ///
    /// Contrary to [`load`](Self::load), it doesn't require a mutable reference; as a consequence,
    /// the cached Arc is not updated when the latest Arc is loaded.
    ///
    /// This method is intended for workflows where the atomic Arc is unlikely to be updated after
    /// the cache construction.
    #[inline]
    pub fn load_shared(&self) -> CachedOrReloaded<'_, A::Arc, A::Domain> {
        self.inner.atomic_arc().load_cached_or_reload(&self.cached)
    }
}

impl<A: AtomicArcRef<Arc = Option<B>>, B: NonNullArcPtr> Cache<A> {
    /// Returns a reference the cached Arc, updating it when it is outdated, as an `Option`.
    ///
    /// See [`load`](Self::load).
    #[inline]
    pub fn load_opt(&mut self) -> Option<&B> {
        self.load().as_ref()
    }
}

impl<A: AtomicArcRef> From<A> for Cache<A> {
    fn from(value: A) -> Self {
        Self::new(value)
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use alloc::sync::Arc;

    use crate::{atomic::AtomicArcPtr, domain, write_policy::Concurrent, AtomicArc, Cache};

    #[test]
    fn cache() {
        domain!(TestDomain(slots = 1));
        let atomic_arc = Arc::new(AtomicArc::<usize, TestDomain>::from(0));
        let mut cache = Cache::new(atomic_arc);
        assert_eq!(**cache.load(), 0);
        cache.inner().store(1.into());
        assert_eq!(**cache.load(), 1);
        assert_eq!(*cache.cached, 1);
    }

    #[test]
    fn cache_option() {
        domain!(TestDomain(slots = 1));
        let atomic_arc =
            Arc::new(AtomicArcPtr::<Option<Arc<usize>>, TestDomain, Concurrent>::from(0));
        let mut cache = Cache::new(atomic_arc);
        assert_eq!(**cache.load_opt().unwrap(), 0);
        cache.inner().store(None);
        assert!(cache.load_opt().is_none());
        assert_eq!(cache.cached, None);
    }
}

//! Cache wrapper to optimize atomic loads of `Arc`-like pointers.

use core::ops::Deref;

use crate::{
    arc::{ArcPtr, NonNullArcPtr},
    atomic::{AtomicArcPtr, AtomicOptionArcPtr, CachedOrReloaded},
    domain::Domain,
    write_policy::WritePolicy,
};

/// A type that dereferences to an [`AtomicArc`](AtomicArcPtr).
pub trait AtomicArcRef {
    /// The concrete Arc pointer used by the `AtomicArc`.
    type Arc;
    /// The owned Arc pointer loaded from the `AtomicArc`.
    ///
    /// It can be `Option<Self::Arc>`.
    type Owned;
    /// The value returned by [`AtomicArc::load_cached`](AtomicArcPtr::load_cached).
    type LoadCached<'a>
    where
        Self::Arc: 'a;
    /// The value returned by
    /// [`AtomicArc::load_cached_or_reload`](AtomicArcPtr::load_cached_or_reload).
    type LoadCachedOrReload<'a>
    where
        Self::Arc: 'a;
    /// Load an owned Arc pointer.
    ///
    /// See [`AtomicArc::load_owned`](AtomicArcPtr::load_owned).
    fn load_owned(&self) -> Self::Owned;
    /// Load an Arc pointer using a given cached value, updating it if necessary.
    ///
    /// See [`AtomicArc::load_cached`](AtomicArcPtr::load_cached).
    fn load_cached<'a>(&self, cached: &'a mut Self::Owned) -> Self::LoadCached<'a>;
    /// Load an Arc pointer using a given cached value, updating it if necessary.
    ///
    /// See [`AtomicArc::load_cached`](AtomicArcPtr::load_cached).
    fn load_cached_or_reload<'a>(&self, cached: &'a Self::Owned) -> Self::LoadCachedOrReload<'a>;
}

impl<A: ArcPtr, D: Domain, W: WritePolicy> AtomicArcRef for AtomicArcPtr<A, D, W> {
    type Arc = A;
    type Owned = A;
    type LoadCached<'a>
        = &'a A
    where
        Self::Arc: 'a;
    type LoadCachedOrReload<'a>
        = CachedOrReloaded<'a, A>
    where
        Self::Arc: 'a;
    #[inline]
    fn load_owned(&self) -> Self::Owned {
        self.load_owned()
    }
    #[inline(always)]
    fn load_cached<'a>(&self, cached: &'a mut Self::Owned) -> Self::LoadCached<'a> {
        self.load_cached(cached)
    }
    #[inline(always)]
    fn load_cached_or_reload<'a>(&self, cached: &'a Self::Owned) -> Self::LoadCachedOrReload<'a> {
        self.load_cached_or_reload(cached)
    }
}

impl<A: NonNullArcPtr, D: Domain, W: WritePolicy> AtomicArcRef for AtomicOptionArcPtr<A, D, W> {
    type Arc = A;
    type Owned = Option<A>;
    type LoadCached<'a>
        = Option<&'a A>
    where
        Self::Arc: 'a;
    type LoadCachedOrReload<'a>
        = Option<CachedOrReloaded<'a, A>>
    where
        Self::Arc: 'a;
    #[inline]
    fn load_owned(&self) -> Self::Owned {
        self.load_owned()
    }
    #[inline(always)]
    fn load_cached<'a>(&self, cached: &'a mut Self::Owned) -> Self::LoadCached<'a> {
        self.load_cached(cached)
    }
    fn load_cached_or_reload<'a>(&self, cached: &'a Self::Owned) -> Self::LoadCachedOrReload<'a> {
        self.load_cached_or_reload(cached)
    }
}

impl<T: Deref> AtomicArcRef for T
where
    T::Target: AtomicArcRef,
{
    type Arc = <T::Target as AtomicArcRef>::Arc;
    type Owned = <T::Target as AtomicArcRef>::Owned;
    type LoadCached<'a>
        = <T::Target as AtomicArcRef>::LoadCached<'a>
    where
        Self::Arc: 'a;
    type LoadCachedOrReload<'a>
        = <T::Target as AtomicArcRef>::LoadCachedOrReload<'a>
    where
        Self::Arc: 'a;
    #[inline]
    fn load_owned(&self) -> Self::Owned {
        (**self).load_owned()
    }
    #[inline]
    fn load_cached<'a>(&self, cached: &'a mut Self::Owned) -> Self::LoadCached<'a> {
        (**self).load_cached(cached)
    }
    #[inline]
    fn load_cached_or_reload<'a>(&self, cached: &'a Self::Owned) -> Self::LoadCachedOrReload<'a> {
        (**self).load_cached_or_reload(cached)
    }
}

/// A cache for a shared [`AtomicArc`](AtomicArcPtr).
///
/// Built as a wrapper around [`AtomicArc::load_cached`](AtomicArcPtr::load_cached),
/// it essentially makes loads of up-to-date Arcs free, but requires a mutable reference.
///
/// As the cache stores the latest loaded Arc, it can delay its reclamation until a new Arc
/// is loaded.
///
/// # Examples
///
/// ```rust
/// # use std::sync::Arc;
/// # hazarc::domain!(Domain(8));
/// # type AtomicArc<T> = hazarc::AtomicArc<T, Domain>;
/// let atomic_arc = Arc::new(AtomicArc::<usize>::from(0));
/// let mut cache = hazarc::Cache::new(atomic_arc);
/// assert_eq!(**cache.load(), 0);
/// ```
///
/// It also works with [`AtomicOptionArc`](AtomicOptionArcPtr).
///
/// ```rust
/// # use std::sync::Arc;
/// # hazarc::domain!(Domain(8));
/// # type AtomicOptionArc<T> = hazarc::AtomicOptionArc<T, Domain>;
/// let atomic_arc = Arc::new(AtomicOptionArc::<usize>::none());
/// let mut cache = hazarc::Cache::new(atomic_arc);
/// assert_eq!(cache.load(), None);
/// ```
#[derive(Debug, Clone)]
pub struct Cache<A: AtomicArcRef> {
    inner: A,
    cached: A::Owned,
}

impl<A: AtomicArcRef> Cache<A> {
    /// Constructs a new `Cache`, loading and storing the up-to-date Arc.
    #[inline]
    pub fn new(inner: A) -> Self {
        let cached = inner.load_owned();
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
    pub fn load(&mut self) -> A::LoadCached<'_> {
        self.inner.load_cached(&mut self.cached)
    }

    /// Returns a reference to the cached Arc if it is up-to-date, or loads the latest Arc.
    ///
    /// Contrary to [`load`](Self::load), it doesn't require a mutable reference; as a consequence,
    /// the cached Arc is not updated when the latest Arc is loaded.
    ///
    /// This method is intended for workflows where the atomic Arc is unlikely to be updated after
    /// the cache construction.
    #[inline]
    pub fn load_shared(&self) -> A::LoadCachedOrReload<'_> {
        self.inner.load_cached_or_reload(&self.cached)
    }
}

impl<A: AtomicArcRef> From<A> for Cache<A> {
    fn from(value: A) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;

    use crate::{domain, AtomicArc, AtomicOptionArc, Cache};

    #[test]
    fn cache() {
        domain!(TestDomain(1));
        let atomic_arc = Arc::new(AtomicArc::<usize, TestDomain>::from(0));
        let mut cache = Cache::new(atomic_arc);
        assert_eq!(**cache.load(), 0);
        cache.inner().store(1.into());
        assert_eq!(**cache.load(), 1);
        assert_eq!(*cache.cached, 1);
    }

    #[test]
    fn cache_option() {
        domain!(TestDomain(1));
        let atomic_arc = Arc::new(AtomicOptionArc::<usize, TestDomain>::from(0));
        let mut cache = Cache::new(atomic_arc);
        assert_eq!(**cache.load().unwrap(), 0);
        cache.inner().store(None);
        assert!(cache.load().is_none());
        assert_eq!(cache.cached, None);
    }
}

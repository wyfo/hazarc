use core::ops::Deref;

use crate::{
    arc::{ArcPtr, NonNullPtr},
    atomic::{AtomicArcPtr, AtomicOptionArcPtr},
    domain::Domain,
    write_policy::WritePolicy,
};

pub trait AtomicArcRef {
    type Arc;
    type Cached;
    type Load<'a>
    where
        Self::Arc: 'a;
    fn load_owned(&self) -> Self::Cached;
    fn load_cached<'a>(&self, cached: &'a mut Self::Cached) -> Self::Load<'a>;
}

impl<A: ArcPtr, D: Domain, W: WritePolicy> AtomicArcRef for AtomicArcPtr<A, D, W> {
    type Arc = A;
    type Cached = A;
    type Load<'a>
        = &'a A
    where
        Self::Arc: 'a;
    #[inline]
    fn load_owned(&self) -> Self::Cached {
        self.load_owned()
    }
    #[inline]
    fn load_cached<'a>(&self, cached: &'a mut Self::Cached) -> Self::Load<'a> {
        self.load_cached(cached)
    }
}

impl<A: ArcPtr + NonNullPtr, D: Domain, W: WritePolicy> AtomicArcRef
    for AtomicOptionArcPtr<A, D, W>
{
    type Arc = A;
    type Cached = Option<A>;
    type Load<'a>
        = Option<&'a A>
    where
        Self::Arc: 'a;
    #[inline]
    fn load_owned(&self) -> Self::Cached {
        self.load_owned()
    }
    #[inline]
    fn load_cached<'a>(&self, cached: &'a mut Self::Cached) -> Self::Load<'a> {
        self.load_cached(cached)
    }
}

impl<T: Deref> AtomicArcRef for T
where
    T::Target: AtomicArcRef,
{
    type Arc = <T::Target as AtomicArcRef>::Arc;
    type Cached = <T::Target as AtomicArcRef>::Cached;
    type Load<'a>
        = <T::Target as AtomicArcRef>::Load<'a>
    where
        Self::Arc: 'a;
    #[inline]
    fn load_owned(&self) -> Self::Cached {
        (**self).load_owned()
    }
    #[inline]
    fn load_cached<'a>(&self, cached: &'a mut Self::Cached) -> Self::Load<'a> {
        (**self).load_cached(cached)
    }
}

// Arc parameter is necessary for `load` method disambiguation.
#[derive(Debug, Clone)]
pub struct Cache<A: AtomicArcRef> {
    inner: A,
    cached: A::Cached,
}

impl<A: AtomicArcRef> Cache<A> {
    #[inline]
    pub fn new(inner: A) -> Self {
        let cached = inner.load_owned();
        Self { inner, cached }
    }

    pub fn inner(&self) -> &A {
        &self.inner
    }

    pub fn into_inner(self) -> A {
        self.inner
    }

    #[inline]
    pub fn load(&mut self) -> A::Load<'_> {
        self.inner.load_cached(&mut self.cached)
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

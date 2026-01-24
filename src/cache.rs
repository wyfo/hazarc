use core::ops::Deref;

use crate::{
    arc::{ArcPtr, NonNullPtr},
    atomic::{AtomicArcPtr, AtomicOptionArcPtr},
    domain::Domain,
};

pub trait Cacheable {
    type Arc;
    type Cached;
    type Load<'a>
    where
        Self::Arc: 'a;
    fn load_owned(&self) -> Self::Cached;
    fn load_cached<'a>(&self, cached: &'a mut Self::Cached) -> Self::Load<'a>;
    fn load_cached_relaxed<'a>(&self, cached: &'a mut Self::Cached) -> Self::Load<'a>;
}

impl<A: ArcPtr, L: Domain> Cacheable for AtomicArcPtr<A, L> {
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
    #[inline]
    fn load_cached_relaxed<'a>(&self, cached: &'a mut Self::Cached) -> Self::Load<'a> {
        self.load_cached_relaxed(cached)
    }
}

impl<A: ArcPtr + NonNullPtr, L: Domain> Cacheable for AtomicOptionArcPtr<A, L> {
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
    #[inline]
    fn load_cached_relaxed<'a>(&self, cached: &'a mut Self::Cached) -> Self::Load<'a> {
        self.load_cached_relaxed(cached)
    }
}

impl<T: Deref> Cacheable for T
where
    T::Target: Cacheable,
{
    type Arc = <T::Target as Cacheable>::Arc;
    type Cached = <T::Target as Cacheable>::Cached;
    type Load<'a>
        = <T::Target as Cacheable>::Load<'a>
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
    #[inline]
    fn load_cached_relaxed<'a>(&self, cached: &'a mut Self::Cached) -> Self::Load<'a> {
        (**self).load_cached_relaxed(cached)
    }
}

// Arc parameter is necessary for `load` method disambiguation.
#[derive(Debug, Clone)]
pub struct Cache<A: Cacheable> {
    inner: A,
    cached: A::Cached,
}

impl<A: Cacheable> Cache<A> {
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

    #[inline]
    pub fn load_relaxed(&mut self) -> A::Load<'_> {
        self.inner.load_cached_relaxed(&mut self.cached)
    }
}

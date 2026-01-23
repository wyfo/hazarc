use core::{fmt, ops::Deref};

use crate::{
    arc::{ArcPtr, NonNullPtr},
    atomic::{AtomicArcPtr, AtomicOptionArcPtr},
    domain::Domain,
};

pub trait AtomicArcRef {
    type Arc: ArcPtr;
    type BorrowList: Domain;
    fn atomic_arc(&self) -> &AtomicArcPtr<Self::Arc, Self::BorrowList>;
}

impl<A: ArcPtr, L: Domain> AtomicArcRef for AtomicArcPtr<A, L> {
    type Arc = A;
    type BorrowList = L;
    #[inline]
    fn atomic_arc(&self) -> &AtomicArcPtr<Self::Arc, Self::BorrowList> {
        self
    }
}

impl<A: ArcPtr + NonNullPtr, L: Domain> AtomicArcRef for AtomicOptionArcPtr<A, L> {
    type Arc = Option<A>;
    type BorrowList = L;
    #[inline]
    fn atomic_arc(&self) -> &AtomicArcPtr<Self::Arc, Self::BorrowList> {
        self.inner()
    }
}

impl<T: Deref> AtomicArcRef for T
where
    T::Target: AtomicArcRef,
{
    type Arc = <T::Target as AtomicArcRef>::Arc;
    type BorrowList = <T::Target as AtomicArcRef>::BorrowList;
    #[inline]
    fn atomic_arc(&self) -> &AtomicArcPtr<Self::Arc, Self::BorrowList> {
        (**self).atomic_arc()
    }
}

// Arc parameter is necessary for `load` method disambiguation.
#[derive(Debug, Clone)]
pub struct ArcCache<A: AtomicArcRef> {
    atomic_arc: A,
    cached: A::Arc,
}

impl<A: AtomicArcRef> ArcCache<A> {
    #[inline]
    pub fn new(atomic_arc: A) -> Self {
        let cached = atomic_arc.atomic_arc().load_owned();
        Self { atomic_arc, cached }
    }

    pub fn atomic_arc(&self) -> &A {
        &self.atomic_arc
    }

    pub fn into_atomic_arc(self) -> A {
        self.atomic_arc
    }

    #[inline]
    pub fn load(&mut self) -> &A::Arc {
        self.atomic_arc.atomic_arc().load_cached(&mut self.cached)
    }

    #[inline]
    pub fn load_relaxed(&mut self) -> &A::Arc {
        self.atomic_arc
            .atomic_arc()
            .load_cached_relaxed(&mut self.cached)
    }
}

// Arc parameter is necessary for `load` method disambiguation.
#[derive(Clone)]
pub struct OptionArcCache<A: AtomicArcRef>(ArcCache<A>);

impl<A: AtomicArcRef<Arc = Option<Arc>>, Arc> OptionArcCache<A> {
    #[inline]
    pub fn new(inner: A) -> Self {
        Self(ArcCache::new(inner))
    }

    pub fn inner(&self) -> &ArcCache<A> {
        &self.0
    }

    pub fn into_inner(self) -> ArcCache<A> {
        self.0
    }

    #[inline]
    pub fn load(&mut self) -> Option<&Arc> {
        self.0.load().as_ref()
    }

    #[inline]
    pub fn load_relaxed(&mut self) -> Option<&Arc> {
        self.0.load_relaxed().as_ref()
    }
}

impl<A: AtomicArcRef<Arc: fmt::Debug> + fmt::Debug> fmt::Debug for OptionArcCache<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OptionArcCache").field(&self.0).finish()
    }
}

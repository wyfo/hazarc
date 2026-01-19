use core::ops::Deref;

use crate::{
    borrow_list::StaticBorrowList,
    generic::{ArcPtr, AtomicArcPtr, NonNullPtr},
};

pub trait AtomicArcRef {
    type Arc: ArcPtr;
    type BorrowList: StaticBorrowList;
    fn atomic_arc(&self) -> &AtomicArcPtr<Self::Arc, Self::BorrowList>;
}

impl<A: ArcPtr, L: StaticBorrowList, T: Deref<Target = AtomicArcPtr<A, L>>> AtomicArcRef for T {
    type Arc = A;
    type BorrowList = L;
    fn atomic_arc(&self) -> &AtomicArcPtr<Self::Arc, Self::BorrowList> {
        self
    }
}

#[derive(Debug, Clone)]
pub struct ArcCache<A: AtomicArcRef, Arc = <A as AtomicArcRef>::Arc> {
    inner: A,
    cached: Arc,
}

impl<A: AtomicArcRef> ArcCache<A> {
    pub fn new(inner: A) -> Self {
        let cached = inner.atomic_arc().load_owned();
        Self { inner, cached }
    }

    pub fn inner(&self) -> &A {
        &self.inner
    }

    pub fn into_inner(self) -> A {
        self.inner
    }
}

impl<A: AtomicArcRef<Arc = Arc>, Arc: ArcPtr + NonNullPtr> ArcCache<A, Arc> {
    pub fn load(&mut self) -> &A::Arc {
        self.inner.atomic_arc().load_cached(&mut self.cached)
    }
}

impl<A: AtomicArcRef<Arc = Option<Arc>>, Arc: ArcPtr + NonNullPtr> ArcCache<A, Option<Arc>> {
    pub fn load(&mut self) -> Option<&Arc> {
        self.inner.atomic_arc().load_cached(&mut self.cached)
    }
}

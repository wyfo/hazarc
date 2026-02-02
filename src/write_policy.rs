//! Write concurrency model for the `AtomicArc` algorithm.

/// Generic parameter of [`AtomicArc`](crate::atomic::AtomicArcPtr) which specifies
/// the assumption about concurrent writes.
///
/// Handling concurrent writes adds some overhead and implications about
/// [wait-freedom](crate#wait-freedom).
pub trait WritePolicy: private::WritePolicy + Send + Sync + 'static {}

/// Concurrent [`WritePolicy`].
///
/// Writes on a given [`AtomicArc`](crate::atomic::AtomicArcPtr) can be concurrent.
/// Compared to [`Serialized`], this policy adds a small overhead to the non-critical
/// path of reads, and a larger overhead to writes on 32-bit platforms.
#[derive(Debug)]
pub struct Concurrent;
impl WritePolicy for Concurrent {}

/// Serialized [`WritePolicy`].
///
/// Writes on a given [`AtomicArc`](crate::atomic::AtomicArcPtr) should be serialized
/// — with a mutex, a MPSC task, etc. Concurrent writes are still safe with this policy,
/// but can provoke non-monotonic reads, i.e. a subsequent load may observe an older
/// value than a previous load.
#[derive(Debug)]
pub struct Serialized;
impl WritePolicy for Serialized {}

mod private {
    use crate::write_policy::{Concurrent, Serialized};

    pub trait WritePolicy {
        const CONCURRENT: bool;
    }
    impl WritePolicy for Concurrent {
        const CONCURRENT: bool = true;
    }
    impl WritePolicy for Serialized {
        const CONCURRENT: bool = false;
    }
}

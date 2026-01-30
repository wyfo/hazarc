pub trait WritePolicy: private::WritePolicy + Send + Sync + 'static {}

#[derive(Debug)]
pub struct Concurrent;
impl WritePolicy for Concurrent {}

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

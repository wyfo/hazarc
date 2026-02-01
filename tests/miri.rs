#![allow(clippy::duplicate_mod)]

#[path = "."]
mod concurrent_8_slots {
    #[allow(unused_imports)]
    use hazarc::write_policy::Concurrent as WritePolicy;
    #[allow(dead_code)]
    const SLOTS: usize = 8;

    #[path = "common/mod.rs"]
    mod common;
    #[path = "concurrent/mod.rs"]
    mod common_concurrent;
}

#[path = "."]
mod concurrent_1_slot {
    #[allow(unused_imports)]
    use hazarc::write_policy::Concurrent as WritePolicy;
    #[allow(dead_code)]
    const SLOTS: usize = 1;

    #[path = "common/mod.rs"]
    mod common;
    #[path = "concurrent/mod.rs"]
    mod common_concurrent;
}

#[path = "."]
mod concurrent_0_slot {
    #[allow(unused_imports)]
    use hazarc::write_policy::Concurrent as WritePolicy;
    #[allow(dead_code)]
    const SLOTS: usize = 0;

    #[path = "common/mod.rs"]
    mod common;
    #[path = "concurrent/mod.rs"]
    mod common_concurrent;
}

#[path = "."]
mod serialized_8_slots {
    #[allow(unused_imports)]
    use hazarc::write_policy::Serialized as WritePolicy;
    #[allow(dead_code)]
    const SLOTS: usize = 8;

    #[path = "common/mod.rs"]
    mod common;
}

#[path = "."]
mod serialized_1_slot {
    #[allow(unused_imports)]
    use hazarc::write_policy::Serialized as WritePolicy;
    #[allow(dead_code)]
    const SLOTS: usize = 1;

    #[path = "common/mod.rs"]
    mod common;
}

#[path = "."]
mod serialized_0_slot {
    #[allow(unused_imports)]
    use hazarc::write_policy::Serialized as WritePolicy;
    #[allow(dead_code)]
    const SLOTS: usize = 0;

    #[path = "common/mod.rs"]
    mod common;
}

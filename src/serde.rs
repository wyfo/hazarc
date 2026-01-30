use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    arc::{ArcPtr, NonNullPtr},
    atomic::{ArcPtrBorrow, AtomicArcPtr, AtomicOptionArcPtr},
    cache::AtomicArcRef,
    domain::Domain,
    write_policy::WritePolicy,
    Cache,
};

impl<A: ArcPtr + Serialize, D: Domain, W: WritePolicy> Serialize for AtomicArcPtr<A, D, W> {
    fn serialize<Ser: Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.load().serialize(serializer)
    }
}

impl<A: ArcPtr + NonNullPtr + Serialize, D: Domain, W: WritePolicy> Serialize
    for AtomicOptionArcPtr<A, D, W>
{
    fn serialize<Ser: Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.load().serialize(serializer)
    }
}

impl<A: ArcPtr + Serialize> Serialize for ArcPtrBorrow<A> {
    fn serialize<Ser: Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        (**self).serialize(serializer)
    }
}

impl<A: AtomicArcRef + Serialize> Serialize for Cache<A> {
    fn serialize<Ser: Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.inner().serialize(serializer)
    }
}

impl<'de, A: ArcPtr + Deserialize<'de>, D: Domain, W: WritePolicy> Deserialize<'de>
    for AtomicArcPtr<A, D, W>
{
    fn deserialize<De: Deserializer<'de>>(deserializer: De) -> Result<Self, De::Error> {
        Ok(Self::new(A::deserialize(deserializer)?))
    }
}

impl<'de, A: ArcPtr + NonNullPtr + Deserialize<'de>, D: Domain, W: WritePolicy> Deserialize<'de>
    for AtomicOptionArcPtr<A, D, W>
{
    fn deserialize<De: Deserializer<'de>>(deserializer: De) -> Result<Self, De::Error> {
        Ok(Self::new(Option::<A>::deserialize(deserializer)?))
    }
}

impl<'de, A: ArcPtr + Deserialize<'de>> Deserialize<'de> for ArcPtrBorrow<A> {
    fn deserialize<De: Deserializer<'de>>(deserializer: De) -> Result<Self, De::Error> {
        Ok(Self::from(A::deserialize(deserializer)?))
    }
}

impl<'de, A: AtomicArcRef + Deserialize<'de>> Deserialize<'de> for Cache<A> {
    fn deserialize<De: Deserializer<'de>>(deserializer: De) -> Result<Self, De::Error> {
        Ok(Self::new(A::deserialize(deserializer)?))
    }
}

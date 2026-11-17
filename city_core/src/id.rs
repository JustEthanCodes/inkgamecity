//! Generational, serde-stable IDs. Bevy `Entity` is never serialized across saves;
//! all cross-system references and persistence use these explicit ids.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! def_id {
    ($name:ident, $inner:ty) => {
        #[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(pub $inner);
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl $name {
            pub const INVALID: Self = Self(<$inner>::MAX);
            #[inline]
            pub const fn is_valid(self) -> bool {
                self.0 != <$inner>::MAX
            }
            #[inline]
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

def_id!(BuildingId, u32);
def_id!(CitizenId, u32);
def_id!(HouseholdId, u32);
def_id!(CompanyId, u32);
def_id!(LotId, u32);
def_id!(LaneNodeId, u32);
def_id!(LaneEdgeId, u32);
def_id!(RoadId, u32);
def_id!(UtilityNodeId, u32);
def_id!(VehicleId, u32);
def_id!(PathId, u32);
def_id!(DistrictId, u16);
def_id!(PlayerId, u64);

impl PlayerId {
    pub const LOCAL: Self = Self(0);
    pub const SERVER: Self = Self(u64::MAX);
}

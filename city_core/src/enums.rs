//! Shared domain enums. Kept `repr(u8)` so dense arrays and on-disk chunks stay compact.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ZoneKind {
    ResidentialLow = 0,
    ResidentialMed = 1,
    ResidentialHigh = 2,
    CommercialLow = 3,
    CommercialHigh = 4,
    Industrial = 5,
    Office = 6,
    MixedUseLow = 7,
    MixedUseMid = 8,
    MixedUseHigh = 9,
    Farm = 10,
    Quarry = 11,
    Landfill = 12,
    Park = 13,
    Forestry = 14,
    Special = 15,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Education {
    Uneducated = 0,
    PoorlyEducated = 1,
    Educated = 2,
    WellEducated = 3,
    HighlyEducated = 4,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum IndustrySector {
    Agriculture = 0,
    Forestry = 1,
    OreExtraction = 2,
    OilExtraction = 3,
    Steel = 4,
    Petrochemicals = 5,
    Electronics = 6,
    FoodProcessing = 7,
    Retail = 8,
    Office = 9,
    FinancialServices = 10,
    Tourism = 11,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Service {
    Power = 0,
    Water = 1,
    Sewage = 2,
    Telecom = 3,
    Trash = 4,
    Fire = 5,
    Police = 6,
    Health = 7,
    Education = 8,
    Transit = 9,
    Parks = 10,
    SnowPlow = 11,
}

pub const SERVICE_COUNT: usize = 12;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TravelMode {
    Walk = 0,
    Bike = 1,
    Car = 2,
    Bus = 3,
    Tram = 4,
    Subway = 5,
    CommuterRail = 6,
    Ferry = 7,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum VehicleClass {
    Car = 0,
    Taxi = 1,
    Bus = 2,
    Tram = 3,
    Emergency = 4,
    Freight = 5,
    Service = 6,
}

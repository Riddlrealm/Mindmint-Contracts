use soroban_sdk::{contracterror, contracttype, Address, Map, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PriceFeedError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    PairNotFound = 4,
    ProviderNotFound = 5,
    ProviderAlreadyExists = 6,
    StalePrice = 7,
    InsufficientProviders = 8,
    InvalidPrice = 9,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceFeed {
    pub pair_id: Symbol,
    pub token_a: Address,
    pub token_b: Address,
    pub providers: Vec<Address>,
    pub prices: Map<Address, i128>,
    pub last_updated: u64,
    pub median_price: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceSnapshot {
    pub median_price: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub stale_threshold: u64,
}

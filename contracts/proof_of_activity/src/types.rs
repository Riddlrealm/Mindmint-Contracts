use soroban_sdk::{contracttype, Address, Symbol, Vec, Map};

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
#[contracttype]
pub enum ActivityType {
    PuzzleSolved = 0,
    TournamentCompleted = 1,
    WaveContributed = 2,
}

// Use Map for storage instead of custom structs
pub type ActivityProofData = Map<Symbol, soroban_sdk::Val>;
pub type OracleConfigData = Map<Symbol, soroban_sdk::Val>;

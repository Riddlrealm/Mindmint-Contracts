use soroban_sdk::{contracttype, Address, Vec, u32};

/// Status of a co-creation collaboration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoCreationStatus {
    /// Initial draft state, awaiting signatures
    Draft,
    /// Signatures being collected
    PendingSignatures,
    /// All signatures collected and published
    Published,
}

/// Creator with their royalty share in basis points (10000 = 100%)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatorShare {
    pub address: Address,
    pub share_bps: u32, // Basis points (0-10000)
}

/// Co-creation collaboration for a puzzle
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoCreation {
    /// Unique identifier for this co-creation
    pub id: u64,
    /// Puzzle ID being co-created
    pub puzzle_id: u64,
    /// List of creators and their shares
    pub creators: Vec<CreatorShare>,
    /// Current status
    pub status: CoCreationStatus,
    /// Addresses that have signed
    pub signatures: Vec<Address>,
    /// Timestamp when created
    pub created_at: u64,
    /// Timestamp when published
    pub published_at: Option<u64>,
}

/// Data keys for storage
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Co-creation by ID
    CoCreation(u64),
    /// Next co-creation ID counter
    NextCoCreationId,
    /// Whether an address has signed a co-creation
    HasSigned(u64, Address),
}

/// Errors that can occur in the contract
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoCreationError {
    /// Co-creation not found
    NotFound = 1,
    /// Invalid share (must be 0-10000 basis points)
    InvalidShare = 2,
    /// Shares must sum to exactly 10000 basis points
    InvalidShareSum = 3,
    /// At least one creator required
    NoCreators = 4,
    /// Duplicate creator address
    DuplicateCreator = 5,
    /// Not a creator
    NotCreator = 6,
    /// Already signed
    AlreadySigned = 7,
    /// All signatures not yet collected
    NotAllSigned = 8,
    /// Already published
    AlreadyPublished = 9,
    /// Cannot sign published co-creation
    AlreadyPublishedSign = 10,
    /// Cannot withdraw from published co-creation
    AlreadyPublishedWithdraw = 11,
    /// Invalid royalty amount
    InvalidAmount = 12,
    /// Not authorized (not royalty oracle)
    Unauthorized = 13,
}

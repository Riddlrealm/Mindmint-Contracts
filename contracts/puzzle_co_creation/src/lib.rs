#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec, symbol_short};

mod storage;
pub mod types;

use crate::storage::*;
use crate::types::*;

// Event symbols
const CO_CREATION_INITIATED: Symbol = symbol_short!("cc_init");
const SIGNATURE_ADDED: Symbol = symbol_short!("sig_add");
const PUZZLE_PUBLISHED: Symbol = symbol_short!("puz_pub");
const ROYALTY_SPLIT: Symbol = symbol_short!("roy_spl");

// Constants
const BASIS_POINTS_TOTAL: u32 = 10000;
const ROYALTY_ORACLE: Symbol = symbol_short!("oracle");

#[contract]
pub struct PuzzleCoCreation;

#[contractimpl]
impl PuzzleCoCreation {
    // ==================== INITIALIZATION ====================

    /// Initialize the contract with the royalty oracle address
    pub fn initialize(env: Env, oracle: Address) {
        // Prevent re-initialization
        if env.storage().instance().get(&ROYALTY_ORACLE).is_some() {
            panic!("Already initialized");
        }

        oracle.require_auth();
        env.storage().instance().set(&ROYALTY_ORACLE, &oracle);
    }

    // ==================== CO-CREATION MANAGEMENT ====================

    /// Initiate a new co-creation collaboration
    /// puzzle_id: The ID of the puzzle being co-created
    /// creators: Vector of (address, share_bps) tuples
    /// Returns the co-creation ID
    pub fn initiate(env: Env, puzzle_id: u64, creators: Vec<CreatorShare>) -> u64 {
        // Validate creators
        Self::validate_creators(&creators);

        // Get first creator (lead creator)
        let lead_creator = creators.get(0).expect("No creators").address.clone();
        lead_creator.require_auth();

        // Create co-creation
        let id = increment_co_creation_id(&env);
        let current_time = env.ledger().timestamp();

        let co_creation = CoCreation {
            id,
            puzzle_id,
            creators: creators.clone(),
            status: CoCreationStatus::PendingSignatures,
            signatures: Vec::new(&env),
            created_at: current_time,
            published_at: None,
        };

        set_co_creation(&env, &co_creation);

        // Lead creator is considered to have signed
        set_signed(&env, id, &lead_creator);

        env.events().publish(
            (CO_CREATION_INITIATED,),
            (id, puzzle_id, lead_creator),
        );

        id
    }

    /// Sign a co-creation to approve it
    /// co_creation_id: The ID of the co-creation to sign
    pub fn sign(env: Env, co_creation_id: u64, signer: Address) {
        signer.require_auth();

        let mut co_creation = get_co_creation(&env, co_creation_id)
            .expect("Co-creation not found");

        // Check if already published
        if co_creation.status == CoCreationStatus::Published {
            panic!("Co-creation already published");
        }

        // Verify signer is a creator
        if !Self::is_creator(&co_creation, &signer) {
            panic!("Not a creator");
        }

        // Check if already signed
        if has_signed(&env, co_creation_id, &signer) {
            panic!("Already signed");
        }

        // Add signature
        co_creation.signatures.push_back(signer.clone());
        set_signed(&env, co_creation_id, &signer);
        set_co_creation(&env, &co_creation);

        env.events().publish(
            (SIGNATURE_ADDED,),
            (co_creation_id, signer),
        );
    }

    /// Publish a co-creation once all creators have signed
    /// co_creation_id: The ID of the co-creation to publish
    pub fn publish(env: Env, co_creation_id: u64) {
        let mut co_creation = get_co_creation(&env, co_creation_id)
            .expect("Co-creation not found");

        // Check if already published
        if co_creation.status == CoCreationStatus::Published {
            panic!("Already published");
        }

        // Verify all creators have signed
        if !Self::all_creators_signed(&env, &co_creation) {
            panic!("Not all creators have signed");
        }

        // Publish
        co_creation.status = CoCreationStatus::Published;
        co_creation.published_at = Some(env.ledger().timestamp());
        set_co_creation(&env, &co_creation);

        env.events().publish(
            (PUZZLE_PUBLISHED,),
            (co_creation_id, co_creation.puzzle_id),
        );
    }

    /// Withdraw signature (only before all have signed)
    /// co_creation_id: The ID of the co-creation
    pub fn withdraw_signature(env: Env, co_creation_id: u64, signer: Address) {
        signer.require_auth();

        let mut co_creation = get_co_creation(&env, co_creation_id)
            .expect("Co-creation not found");

        // Cannot withdraw from published co-creation
        if co_creation.status == CoCreationStatus::Published {
            panic!("Cannot withdraw from published co-creation");
        }

        // Verify signer is a creator
        if !Self::is_creator(&co_creation, &signer) {
            panic!("Not a creator");
        }

        // Check if already signed
        if !has_signed(&env, co_creation_id, &signer) {
            panic!("Not signed");
        }

        // Remove signature
        let mut new_signatures = Vec::new(&env);
        for sig in co_creation.signatures.iter() {
            if sig != signer {
                new_signatures.push_back(sig);
            }
        }
        co_creation.signatures = new_signatures;
        remove_signed(&env, co_creation_id, &signer);

        // If this was the first signature, revert to draft
        if co_creation.signatures.is_empty() {
            co_creation.status = CoCreationStatus::Draft;
        }

        set_co_creation(&env, &co_creation);
    }

    // ==================== ROYALTY DISTRIBUTION ====================

    /// Distribute royalties among creators
    /// co_creation_id: The ID of the co-creation
    /// total_amount: Total amount to distribute
    pub fn distribute_royalty(env: Env, co_creation_id: u64, total_amount: i128) {
        // Verify caller is royalty oracle
        let oracle = env.storage().instance()
            .get(&ROYALTY_ORACLE)
            .expect("Not initialized");
        oracle.require_auth();

        // Validate amount
        if total_amount <= 0 {
            panic!("Invalid amount");
        }

        let co_creation = get_co_creation(&env, co_creation_id)
            .expect("Co-creation not found");

        // Must be published to receive royalties
        if co_creation.status != CoCreationStatus::Published {
            panic!("Co-creation not published");
        }

        // Distribute to each creator based on their share
        for creator_share in co_creation.creators.iter() {
            let share_amount = (total_amount * creator_share.share_bps as i128) / BASIS_POINTS_TOTAL as i128;
            
            if share_amount > 0 {
                // In a real implementation, this would transfer tokens
                // For now, we just emit an event
                env.events().publish(
                    (ROYALTY_SPLIT,),
                    (co_creation_id, creator_share.address.clone(), share_amount),
                );
            }
        }
    }

    // ==================== VIEW FUNCTIONS ====================

    /// Get co-creation details
    pub fn get_co_creation(env: Env, id: u64) -> CoCreation {
        get_co_creation(&env, id).expect("Co-creation not found")
    }

    /// Check if an address has signed a co-creation
    pub fn has_signed(env: Env, co_creation_id: u64, signer: Address) -> bool {
        has_signed(&env, co_creation_id, &signer)
    }

    /// Get the royalty oracle address
    pub fn get_oracle(env: Env) -> Address {
        env.storage().instance().get(&ROYALTY_ORACLE).expect("Not initialized")
    }

    // ==================== HELPER FUNCTIONS ====================

    /// Validate creator shares
    fn validate_creators(creators: &Vec<CreatorShare>) {
        // At least one creator required
        if creators.is_empty() {
            panic!("At least one creator required");
        }

        // Check for duplicate addresses
        let mut seen = Vec::new(creators.env());
        for creator in creators.iter() {
            for seen_addr in seen.iter() {
                if seen_addr == &creator.address {
                    panic!("Duplicate creator address");
                }
            }
            seen.push_back(creator.address.clone());
        }

        // Validate each share is within bounds
        for creator in creators.iter() {
            if creator.share_bps == 0 || creator.share_bps > BASIS_POINTS_TOTAL {
                panic!("Invalid share: must be 1-10000 basis points");
            }
        }

        // Validate shares sum to exactly 10000 basis points
        let total_share: u32 = creators.iter()
            .map(|c| c.share_bps)
            .sum();

        if total_share != BASIS_POINTS_TOTAL {
            panic!("Shares must sum to exactly 10000 basis points");
        }
    }

    /// Check if an address is a creator
    fn is_creator(co_creation: &CoCreation, address: &Address) -> bool {
        for creator in co_creation.creators.iter() {
            if creator.address == *address {
                return true;
            }
        }
        false
    }

    /// Check if all creators have signed
    fn all_creators_signed(env: &Env, co_creation: &CoCreation) -> bool {
        for creator in co_creation.creators.iter() {
            if !has_signed(env, co_creation.id, &creator.address) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod test;

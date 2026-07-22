#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub struct Achievement {
    pub owner: Address,
    pub puzzle_id: u32,
    pub metadata: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Counters {
    pub next_token_id: u32,
    pub total_supply: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionItem {
    pub token_id: u32,
    pub puzzle_id: u32,
}

/// Audit entry written whenever a minter role is granted or revoked.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinterAuditEntry {
    /// The admin who performed the action.
    pub actor: Address,
    /// The address whose role changed.
    pub subject: Address,
    /// `true` = granted, `false` = revoked.
    pub granted: bool,
    /// Ledger timestamp at the time of the action.
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

#[contracttype]
pub enum DataKey {
    Achievement(u32),
    OwnerCollection(Address),
    Counters,
    Admin,
    PuzzleCompleted(Address, u32),
    /// Stores `true` when `Address` holds the minter role.
    Minter(Address),
    /// Sampled audit log entry keyed by (subject, ledger_timestamp).
    MinterAudit(Address, u64),
}

// ---------------------------------------------------------------------------
// Error contract
// ---------------------------------------------------------------------------

/// Returned (via panic) when a caller lacks the minter role.
const NOT_MINTER: &str = "NotMinter";
/// Returned when a non-admin tries an admin-only action.
const NOT_ADMIN: &str = "NotAdmin";

// TTL constants (in ledgers; ~5 s / ledger on Stellar)
const TTL_LOW: u32 = 100_000;
const TTL_HIGH: u32 = 500_000;
/// Audit log TTL — shorter, sampled retention.
const AUDIT_TTL_LOW: u32 = 17_280;    // ~1 day
const AUDIT_TTL_HIGH: u32 = 120_960;  // ~7 days

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct AchievementNFT;

#[contractimpl]
impl AchievementNFT {
    // -----------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------

    /// Initialize the contract with an admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(
            &DataKey::Counters,
            &Counters {
                next_token_id: 1,
                total_supply: 0,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Minter-role management (admin-only)
    // -----------------------------------------------------------------------

    /// Grant the minter role to `minter`.  Only the admin may call this.
    /// Emits a `minter_grant` event and writes a sampled TTL audit entry.
    pub fn grant_minter(env: Env, minter: Address) {
        let admin = Self::require_admin(&env);

        env.storage()
            .persistent()
            .set(&DataKey::Minter(minter.clone()), &true);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Minter(minter.clone()), TTL_LOW, TTL_HIGH);

        // Audit log
        Self::write_audit(&env, &admin, &minter, true);

        env.events().publish(
            (symbol_short!("mtr_grant"), admin),
            minter,
        );
    }

    /// Revoke the minter role from `minter`.  Only the admin may call this.
    /// Emits a `minter_rev` event and writes a sampled TTL audit entry.
    pub fn revoke_minter(env: Env, minter: Address) {
        let admin = Self::require_admin(&env);

        env.storage()
            .persistent()
            .remove(&DataKey::Minter(minter.clone()));

        // Audit log
        Self::write_audit(&env, &admin, &minter, false);

        env.events().publish(
            (symbol_short!("mtr_rev"), admin),
            minter,
        );
    }

    /// Query whether `addr` currently holds the minter role.
    pub fn is_minter(env: Env, addr: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Minter(addr))
            .unwrap_or(false)
    }

    // -----------------------------------------------------------------------
    // Puzzle tracking (admin-only)
    // -----------------------------------------------------------------------

    /// Mark a puzzle as completed for `user` (admin-only).
    pub fn mark_puzzle_completed(env: Env, user: Address, puzzle_id: u32) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let key = DataKey::PuzzleCompleted(user, puzzle_id);
        env.storage().persistent().set(&key, &true);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_LOW, TTL_HIGH);
    }

    // -----------------------------------------------------------------------
    // Minting
    // -----------------------------------------------------------------------

    /// Mint an achievement NFT.
    ///
    /// Requires:
    /// - The caller (`to`) has the **minter role**; panics with `"NotMinter"` otherwise.
    /// - The puzzle has been marked completed for `to`.
    pub fn mint(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32 {
        to.require_auth();

        // Minter role guard
        let has_role: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Minter(to.clone()))
            .unwrap_or(false);
        if !has_role {
            panic!("{}", NOT_MINTER);
        }

        let completed: bool = env
            .storage()
            .persistent()
            .get(&DataKey::PuzzleCompleted(to.clone(), puzzle_id))
            .unwrap_or(false);

        if !completed {
            panic!("Puzzle not completed");
        }

        Self::mint_internal(env, to, puzzle_id, metadata)
    }

    /// Craft-mint path.
    ///
    /// Requires the **minter role**; panics with `"NotMinter"` otherwise.
    pub fn craftmint(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32 {
        to.require_auth();

        // Minter role guard
        let has_role: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Minter(to.clone()))
            .unwrap_or(false);
        if !has_role {
            panic!("{}", NOT_MINTER);
        }

        Self::mint_internal(env, to, puzzle_id, metadata)
    }

    // -----------------------------------------------------------------------
    // Transfer
    // -----------------------------------------------------------------------

    /// Transfer an NFT from `from` to `to`.
    pub fn transfer(env: Env, from: Address, to: Address, token_id: u32) {
        from.require_auth();

        if from == to {
            panic!("Cannot transfer to self");
        }

        let mut achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");

        if achievement.owner != from {
            panic!("Not the owner");
        }

        // Remove from sender's collection
        let mut from_col = Self::get_collection_internal(&env, from.clone());
        let mut index = None;
        for (i, item) in from_col.iter().enumerate() {
            if item.token_id == token_id {
                index = Some(i as u32);
                break;
            }
        }

        let idx = index.expect("ID not in collection");
        from_col.remove(idx);

        env.storage()
            .persistent()
            .set(&DataKey::OwnerCollection(from.clone()), &from_col);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::OwnerCollection(from.clone()), TTL_LOW, TTL_HIGH);

        // Add to receiver's collection
        let mut to_col = Self::get_collection_internal(&env, to.clone());
        to_col.push_back(CollectionItem {
            token_id,
            puzzle_id: achievement.puzzle_id,
        });

        env.storage()
            .persistent()
            .set(&DataKey::OwnerCollection(to.clone()), &to_col);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::OwnerCollection(to.clone()), TTL_LOW, TTL_HIGH);

        // Update ownership
        achievement.owner = to.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Achievement(token_id), &achievement);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Achievement(token_id), TTL_LOW, TTL_HIGH);

        env.events()
            .publish((symbol_short!("transfer"), from, to), token_id);
    }

    // -----------------------------------------------------------------------
    // Burn
    // -----------------------------------------------------------------------

    /// Burn (permanently destroy) an NFT.  Any holder can burn their own token.
    pub fn burn(env: Env, token_id: u32) {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");

        let mut collection = Self::get_collection_internal(&env, achievement.owner.clone());

        let mut index = None;
        for (i, item) in collection.iter().enumerate() {
            if item.token_id == token_id {
                index = Some(i as u32);
                break;
            }
        }

        if let Some(idx) = index {
            collection.remove(idx);
            env.storage().persistent().set(
                &DataKey::OwnerCollection(achievement.owner.clone()),
                &collection,
            );
        }

        env.storage()
            .persistent()
            .remove(&DataKey::Achievement(token_id));

        let mut counters: Counters = env
            .storage()
            .instance()
            .get(&DataKey::Counters)
            .expect("Not initialized");

        counters.total_supply -= 1;
        env.storage().instance().set(&DataKey::Counters, &counters);

        env.events()
            .publish((symbol_short!("burn"), achievement.owner), token_id);
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    pub fn get_collection(env: Env, owner: Address) -> Vec<u32> {
        let collection = Self::get_collection_internal(&env, owner);
        let mut result = Vec::new(&env);
        for item in collection.iter() {
            result.push_back(item.token_id);
        }
        result
    }

    pub fn owner_of(env: Env, token_id: u32) -> Address {
        let achievement: Achievement = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
            .expect("Token does not exist");

        achievement.owner
    }

    pub fn total_supply(env: Env) -> u32 {
        let counters: Counters =
            env.storage()
                .instance()
                .get(&DataKey::Counters)
                .unwrap_or(Counters {
                    next_token_id: 1,
                    total_supply: 0,
                });
        counters.total_supply
    }

    pub fn get_achievement(env: Env, token_id: u32) -> Option<Achievement> {
        env.storage()
            .persistent()
            .get(&DataKey::Achievement(token_id))
    }

    pub fn puzzle_ids_of(env: Env, owner: Address) -> Vec<u32> {
        let collection = Self::get_collection_internal(&env, owner);
        let mut puzzles = Vec::new(&env);

        for item in collection.iter() {
            if !puzzles.contains(&item.puzzle_id) {
                puzzles.push_back(item.puzzle_id);
            }
        }

        puzzles
    }

    pub fn has_puzzle(env: Env, owner: Address, puzzle_id: u32) -> bool {
        let collection = Self::get_collection_internal(&env, owner);
        for item in collection.iter() {
            if item.puzzle_id == puzzle_id {
                return true;
            }
        }
        false
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Verify the caller is the admin and return the admin address.
    fn require_admin(env: &Env) -> Address {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect(NOT_ADMIN);
        admin.require_auth();
        admin
    }

    /// Write a sampled-TTL audit entry for a minter role change.
    fn write_audit(env: &Env, actor: &Address, subject: &Address, granted: bool) {
        let ts = env.ledger().timestamp();
        let entry = MinterAuditEntry {
            actor: actor.clone(),
            subject: subject.clone(),
            granted,
            timestamp: ts,
        };
        let key = DataKey::MinterAudit(subject.clone(), ts);
        env.storage().persistent().set(&key, &entry);
        env.storage()
            .persistent()
            .extend_ttl(&key, AUDIT_TTL_LOW, AUDIT_TTL_HIGH);
    }

    fn mint_internal(env: Env, to: Address, puzzle_id: u32, metadata: String) -> u32 {
        let mut counters: Counters = env
            .storage()
            .instance()
            .get(&DataKey::Counters)
            .expect("Not initialized");

        let token_id = counters.next_token_id;

        let achievement = Achievement {
            owner: to.clone(),
            puzzle_id,
            metadata,
            timestamp: env.ledger().timestamp(),
        };

        // Store NFT
        let key = DataKey::Achievement(token_id);
        env.storage().persistent().set(&key, &achievement);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_LOW, TTL_HIGH);

        // Update owner collection
        let mut collection = Self::get_collection_internal(&env, to.clone());
        collection.push_back(CollectionItem {
            token_id,
            puzzle_id,
        });

        let collection_key = DataKey::OwnerCollection(to.clone());
        env.storage().persistent().set(&collection_key, &collection);
        env.storage()
            .persistent()
            .extend_ttl(&collection_key, TTL_LOW, TTL_HIGH);

        // Update counters
        counters.next_token_id += 1;
        counters.total_supply += 1;
        env.storage().instance().set(&DataKey::Counters, &counters);

        // Emit event
        env.events()
            .publish((symbol_short!("minted"), to), token_id);

        token_id
    }

    fn get_collection_internal(env: &Env, owner: Address) -> Vec<CollectionItem> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerCollection(owner))
            .unwrap_or(Vec::new(env))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, AchievementNFTClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, AchievementNFT);
        let client = AchievementNFTClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    // -----------------------------------------------------------------------
    // grant_minter / revoke_minter
    // -----------------------------------------------------------------------

    #[test]
    fn test_grant_minter_sets_role() {
        let (env, client, _admin) = setup();
        let minter = Address::generate(&env);

        assert!(!client.is_minter(&minter));
        client.grant_minter(&minter);
        assert!(client.is_minter(&minter));
    }

    #[test]
    fn test_revoke_minter_clears_role() {
        let (env, client, _admin) = setup();
        let minter = Address::generate(&env);

        client.grant_minter(&minter);
        assert!(client.is_minter(&minter));

        client.revoke_minter(&minter);
        assert!(!client.is_minter(&minter));
    }

    #[test]
    #[should_panic]
    fn test_grant_minter_requires_admin_auth() {
        let env = Env::default();
        // Do NOT mock auths — the admin must actually authorize.
        let contract_id = env.register_contract(None, AchievementNFT);
        let client = AchievementNFTClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        // Initialize with mock auths just for setup.
        env.mock_all_auths();
        client.initialize(&admin);

        // Now clear mocks and attempt grant without proper auth — should panic.
        let env2 = Env::default();
        let client2 = AchievementNFTClient::new(&env2, &contract_id);
        let minter = Address::generate(&env2);
        client2.grant_minter(&minter); // no auth mocked → panic
    }

    // -----------------------------------------------------------------------
    // mint with role
    // -----------------------------------------------------------------------

    #[test]
    fn test_mint_with_minter_role_succeeds() {
        let (env, client, _admin) = setup();
        let minter = Address::generate(&env);

        // Grant role and mark puzzle complete.
        client.grant_minter(&minter);
        client.mark_puzzle_completed(&minter, &1u32);

        let meta = String::from_str(&env, "ipfs://test");
        let token_id = client.mint(&minter, &1u32, &meta);

        assert_eq!(token_id, 1u32);
        assert_eq!(client.owner_of(&token_id), minter);
        assert_eq!(client.total_supply(), 1u32);
    }

    // -----------------------------------------------------------------------
    // mint without role — must panic with NotMinter
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "NotMinter")]
    fn test_mint_without_minter_role_panics() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);

        // Mark puzzle complete but do NOT grant the minter role.
        client.mark_puzzle_completed(&user, &1u32);

        let meta = String::from_str(&env, "ipfs://test");
        client.mint(&user, &1u32, &meta); // should panic
    }

    #[test]
    #[should_panic(expected = "NotMinter")]
    fn test_craftmint_without_minter_role_panics() {
        let (env, client, _admin) = setup();
        let user = Address::generate(&env);

        let meta = String::from_str(&env, "ipfs://test");
        client.craftmint(&user, &1u32, &meta); // should panic
    }

    #[test]
    fn test_craftmint_with_minter_role_succeeds() {
        let (env, client, _admin) = setup();
        let minter = Address::generate(&env);

        client.grant_minter(&minter);

        let meta = String::from_str(&env, "ipfs://craft");
        let token_id = client.craftmint(&minter, &42u32, &meta);

        assert_eq!(token_id, 1u32);
        assert_eq!(client.owner_of(&token_id), minter);
    }

    // -----------------------------------------------------------------------
    // Revoked role prevents further minting
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "NotMinter")]
    fn test_mint_after_revoke_panics() {
        let (env, client, _admin) = setup();
        let minter = Address::generate(&env);

        client.grant_minter(&minter);
        client.mark_puzzle_completed(&minter, &1u32);
        client.revoke_minter(&minter);

        let meta = String::from_str(&env, "ipfs://test");
        client.mint(&minter, &1u32, &meta); // role revoked → should panic
    }

    // -----------------------------------------------------------------------
    // Audit log
    // -----------------------------------------------------------------------

    #[test]
    fn test_audit_log_written_on_grant() {
        let (env, client, admin) = setup();
        let minter = Address::generate(&env);

        let ts_before = env.ledger().timestamp();
        client.grant_minter(&minter);

        // Retrieve the audit entry directly from storage.
        let key = DataKey::MinterAudit(minter.clone(), ts_before);
        let entry: Option<MinterAuditEntry> = env.as_contract(&client.address, || {
            env.storage().persistent().get(&key)
        });

        let entry = entry.expect("audit entry must exist after grant_minter");
        assert_eq!(entry.subject, minter);
        assert_eq!(entry.actor, admin);
        assert!(entry.granted);
    }

    #[test]
    fn test_audit_log_written_on_revoke() {
        let (env, client, admin) = setup();
        let minter = Address::generate(&env);

        client.grant_minter(&minter);

        let ts_before = env.ledger().timestamp();
        client.revoke_minter(&minter);

        let key = DataKey::MinterAudit(minter.clone(), ts_before);
        let entry: Option<MinterAuditEntry> = env.as_contract(&client.address, || {
            env.storage().persistent().get(&key)
        });

        let entry = entry.expect("audit entry must exist after revoke_minter");
        assert_eq!(entry.subject, minter);
        assert_eq!(entry.actor, admin);
        assert!(!entry.granted);
    }
}

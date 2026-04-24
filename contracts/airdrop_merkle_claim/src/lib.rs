#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, BytesN, Env, Symbol, Vec,
};

const BASIS_POINTS: i128 = 10_000;

/// Represents an airdrop campaign
#[contracttype]
#[derive(Clone, Debug)]
pub struct AirdropCampaign {
    pub id: u32,
    pub merkle_root: BytesN<32>,
    pub token: Address,
    pub total_allocation: i128,
    pub claimed_count: u32,
    pub claimed_amount: i128,
    pub deadline: u64,
    pub status: u8, // 0 = active, 1 = expired, 2 = cancelled
}

/// Data key enumeration for storage
#[contracttype]
pub enum DataKey {
    Admin,
    CampaignCounter,
    Campaign(u32),
    Claimed(u32, Address), // (campaign_id, address)
}

#[contract]
pub struct AirdropMerkleClaimContract;

#[contractimpl]
impl AirdropMerkleClaimContract {
    /// Initialize the contract with an admin
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::CampaignCounter, &0u32);
    }

    /// Require authentication from the admin
    fn require_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not set");
        admin.require_auth();
    }

    /// Get the current admin
    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not set")
    }

    /// Create a new airdrop campaign
    /// Admin deposits tokens; merkle_root proves eligibility
    pub fn create_campaign(
        env: Env,
        admin: Address,
        merkle_root: BytesN<32>,
        token: Address,
        total_allocation: i128,
        deadline: u64,
    ) -> u32 {
        admin.require_auth();
        Self::require_admin(&env);

        if total_allocation <= 0 {
            panic!("total allocation must be positive");
        }

        let now = env.ledger().timestamp();
        if deadline <= now {
            panic!("deadline must be in the future");
        }

        // Transfer tokens from admin to contract
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&admin, &env.current_contract_address(), &total_allocation);

        // Get next campaign ID
        let counter: u32 = env
            .storage()
            .instance()
            .get(&DataKey::CampaignCounter)
            .unwrap_or(0);
        let campaign_id = counter + 1;

        let campaign = AirdropCampaign {
            id: campaign_id,
            merkle_root,
            token,
            total_allocation,
            claimed_count: 0,
            claimed_amount: 0,
            deadline,
            status: 0, // active
        };

        env.storage()
            .instance()
            .set(&DataKey::Campaign(campaign_id), &campaign);
        env.storage()
            .instance()
            .set(&DataKey::CampaignCounter, &campaign_id);

        // Emit CampaignCreated event
        env.events().publish(
            (Symbol::new(&env, "airdrop"), Symbol::new(&env, "campaign_created")),
            (campaign_id, merkle_root, token, total_allocation, deadline),
        );

        campaign_id
    }

    /// Claim tokens from a campaign using a merkle proof
    /// Each address can only claim once per campaign
    pub fn claim(
        env: Env,
        campaign_id: u32,
        claimer: Address,
        amount: i128,
        merkle_proof: Vec<BytesN<32>>,
    ) {
        claimer.require_auth();

        if amount <= 0 {
            panic!("claim amount must be positive");
        }

        // Get campaign
        let mut campaign: AirdropCampaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .expect("campaign not found");

        let now = env.ledger().timestamp();

        // Check deadline
        if now > campaign.deadline {
            panic!("campaign expired");
        }

        // Check campaign status
        if campaign.status != 0 {
            panic!("campaign not active");
        }

        // Check if already claimed
        if env
            .storage()
            .instance()
            .has(&DataKey::Claimed(campaign_id, claimer.clone()))
        {
            panic!("already claimed");
        }

        // Verify merkle proof
        if !Self::verify_proof(
            env.clone(),
            campaign_id,
            claimer.clone(),
            amount,
            merkle_proof,
        ) {
            panic!("invalid merkle proof");
        }

        // Check sufficient allocation remains
        if campaign.claimed_amount + amount > campaign.total_allocation {
            panic!("insufficient allocation");
        }

        // Mark as claimed
        env.storage()
            .instance()
            .set(&DataKey::Claimed(campaign_id, claimer.clone()), &true);

        // Update campaign stats
        campaign.claimed_count += 1;
        campaign.claimed_amount += amount;
        env.storage()
            .instance()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        // Transfer tokens to claimer
        let token_client = token::Client::new(&env, &campaign.token);
        token_client.transfer(&env.current_contract_address(), &claimer, &amount);

        // Emit TokensClaimed event
        env.events().publish(
            (Symbol::new(&env, "airdrop"), Symbol::new(&env, "tokens_claimed")),
            (campaign_id, claimer, amount),
        );
    }

    /// Verify a merkle proof for eligibility
    /// Leaf is: sha256(address || amount)
    pub fn verify_proof(
        env: Env,
        campaign_id: u32,
        address: Address,
        amount: i128,
        merkle_proof: Vec<BytesN<32>>,
    ) -> bool {
        let campaign: AirdropCampaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .expect("campaign not found");

        // Construct the leaf: hash(address || amount)
        let mut leaf_input = Vec::new(&env);
        // Serialize address bytes (typically 32 bytes)
        let address_bytes = address.to_xdr(&env);
        leaf_input.push_back(address_bytes);
        // Serialize amount as i128 (16 bytes)
        let amount_bytes = amount.to_xdr(&env);
        leaf_input.push_back(amount_bytes);

        // Hash the leaf
        let combined = Self::combine_bytes(&env, leaf_input);
        let mut current_hash = env.crypto().sha256(&combined);

        // Verify proof by hashing up the tree
        let mut i = 0;
        while i < merkle_proof.len() {
            let proof_element = merkle_proof.get(i).unwrap();

            // Combine current hash with proof element
            let mut combined_input = Vec::new(&env);
            combined_input.push_back(current_hash.to_xdr(&env));
            combined_input.push_back(proof_element.to_xdr(&env));

            let combined_bytes = Self::combine_bytes(&env, combined_input);
            current_hash = env.crypto().sha256(&combined_bytes).into();

            i += 1;
        }

        // Compare with merkle root
        current_hash == campaign.merkle_root
    }

    /// Helper function to combine byte arrays
    fn combine_bytes(env: &Env, byte_vecs: Vec<soroban_sdk::Bytes>) -> soroban_sdk::Bytes {
        let mut result = soroban_sdk::Bytes::new(env);
        let mut i = 0;
        while i < byte_vecs.len() {
            let bytes = byte_vecs.get(i).unwrap();
            let mut j = 0;
            while j < bytes.len() {
                result.push_back(bytes.get(j).unwrap());
                j += 1;
            }
            i += 1;
        }
        result
    }

    /// Get campaign details
    pub fn get_campaign(env: Env, campaign_id: u32) -> Option<AirdropCampaign> {
        env.storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
    }

    /// Check if an address has already claimed from a campaign
    pub fn has_claimed(env: Env, campaign_id: u32, address: Address) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::Claimed(campaign_id, address))
    }

    /// Admin reclaims unclaimed tokens after deadline
    pub fn reclaim_unclaimed(env: Env, admin: Address, campaign_id: u32) -> i128 {
        admin.require_auth();
        Self::require_admin(&env);

        let mut campaign: AirdropCampaign = env
            .storage()
            .instance()
            .get(&DataKey::Campaign(campaign_id))
            .expect("campaign not found");

        let now = env.ledger().timestamp();

        // Check deadline has passed
        if now <= campaign.deadline {
            panic!("campaign still active");
        }

        let unclaimed = campaign.total_allocation - campaign.claimed_amount;

        if unclaimed == 0 {
            panic!("no unclaimed tokens");
        }

        // Mark campaign as expired
        campaign.status = 1;
        env.storage()
            .instance()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        // Transfer unclaimed tokens back to admin
        let token_client = token::Client::new(&env, &campaign.token);
        token_client.transfer(
            &env.current_contract_address(),
            &admin,
            &unclaimed,
        );

        // Emit UnclaimedReclaimed event
        env.events().publish(
            (Symbol::new(&env, "airdrop"), Symbol::new(&env, "unclaimed_reclaimed")),
            (campaign_id, unclaimed),
        );

        unclaimed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::*, Bytes};

    #[test]
    fn test_create_campaign() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AirdropMerkleClaimContract);
        let client = AirdropMerkleClaimContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let merkle_root = BytesN::from_array(&env, &[42u8; 32]);
        let total_allocation = 1_000_000i128;
        let deadline = env.ledger().timestamp() + 86400; // 1 day from now

        // Mock token transfer
        env.mock_all_auths();

        client.initialize(&admin);
        let campaign_id = client.create_campaign(
            &admin,
            &merkle_root,
            &token,
            &total_allocation,
            &deadline,
        );

        assert_eq!(campaign_id, 1);

        let campaign = client.get_campaign(&campaign_id).unwrap();
        assert_eq!(campaign.id, 1);
        assert_eq!(campaign.total_allocation, total_allocation);
        assert_eq!(campaign.claimed_count, 0);
        assert_eq!(campaign.claimed_amount, 0);
        assert_eq!(campaign.status, 0);
    }

    #[test]
    fn test_double_claim_rejection() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AirdropMerkleClaimContract);
        let client = AirdropMerkleClaimContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let claimer = Address::generate(&env);
        let token = Address::generate(&env);
        let merkle_root = BytesN::from_array(&env, &[42u8; 32]);

        env.mock_all_auths();

        client.initialize(&admin);
        let campaign_id = client.create_campaign(
            &admin,
            &merkle_root,
            &token,
            &1_000_000i128,
            &(env.ledger().timestamp() + 86400),
        );

        // First claim should succeed
        let amount = 100_000i128;
        let proof = Vec::new(&env);

        // Mark as claimed manually for this test
        env.storage()
            .instance()
            .set(&DataKey::Claimed(campaign_id, claimer.clone()), &true);

        // Second attempt should fail
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.claim(&campaign_id, &claimer, &amount, &proof);
        }));

        assert!(result.is_err());
    }

    #[test]
    fn test_deadline_enforcement() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AirdropMerkleClaimContract);
        let client = AirdropMerkleClaimContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let claimer = Address::generate(&env);
        let token = Address::generate(&env);
        let merkle_root = BytesN::from_array(&env, &[42u8; 32]);

        env.mock_all_auths();

        client.initialize(&admin);

        // Create campaign that expires immediately
        let expired_deadline = env.ledger().timestamp();
        let campaign_id = client.create_campaign(
            &admin,
            &merkle_root,
            &token,
            &1_000_000i128,
            &expired_deadline,
        );

        // Attempt to claim from expired campaign should fail
        let amount = 100_000i128;
        let proof = Vec::new(&env);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.claim(&campaign_id, &claimer, &amount, &proof);
        }));

        assert!(result.is_err());
    }

    #[test]
    fn test_reclaim_after_expiry() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AirdropMerkleClaimContract);
        let client = AirdropMerkleClaimContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let merkle_root = BytesN::from_array(&env, &[42u8; 32]);
        let total_allocation = 1_000_000i128;

        env.mock_all_auths();

        client.initialize(&admin);

        let deadline = env.ledger().timestamp() + 1000; // Expire in 1000 seconds
        let campaign_id = client.create_campaign(
            &admin,
            &merkle_root,
            &token,
            &total_allocation,
            &deadline,
        );

        // Fast forward time past deadline
        env.ledger().with_timestamp(deadline + 1);

        // Admin should be able to reclaim
        let unclaimed = client.reclaim_unclaimed(&admin, &campaign_id);
        assert_eq!(unclaimed, total_allocation);

        // Campaign should be marked as expired
        let campaign = client.get_campaign(&campaign_id).unwrap();
        assert_eq!(campaign.status, 1); // expired
    }

    #[test]
    fn test_invalid_proof_rejection() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AirdropMerkleClaimContract);
        let client = AirdropMerkleClaimContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let claimer = Address::generate(&env);
        let token = Address::generate(&env);
        let merkle_root = BytesN::from_array(&env, &[42u8; 32]);

        env.mock_all_auths();

        client.initialize(&admin);
        let campaign_id = client.create_campaign(
            &admin,
            &merkle_root,
            &token,
            &1_000_000i128,
            &(env.ledger().timestamp() + 86400),
        );

        // Claim with invalid proof should fail
        let amount = 100_000i128;
        let invalid_proof = {
            let mut proof = Vec::new(&env);
            proof.push_back(BytesN::from_array(&env, &[255u8; 32]));
            proof
        };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.claim(&campaign_id, &claimer, &amount, &invalid_proof);
        }));

        assert!(result.is_err());
    }

    #[test]
    fn test_get_campaign() {
        let env = Env::default();
        let contract_id = env.register_contract(None, AirdropMerkleClaimContract);
        let client = AirdropMerkleClaimContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let merkle_root = BytesN::from_array(&env, &[42u8; 32]);
        let total_allocation = 500_000i128;
        let deadline = env.ledger().timestamp() + 172800; // 2 days

        env.mock_all_auths();

        client.initialize(&admin);
        let campaign_id = client.create_campaign(
            &admin,
            &merkle_root,
            &token,
            &total_allocation,
            &deadline,
        );

        let campaign = client.get_campaign(&campaign_id).unwrap();
        assert_eq!(campaign.id, campaign_id);
        assert_eq!(campaign.merkle_root, merkle_root);
        assert_eq!(campaign.token, token);
        assert_eq!(campaign.total_allocation, total_allocation);
        assert_eq!(campaign.deadline, deadline);
    }
}

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, BytesN, Env, Symbol, Vec};

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
    pub status: u32, // 0 = active, 1 = expired, 2 = cancelled
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
        env.storage()
            .instance()
            .set(&DataKey::CampaignCounter, &0u32);
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
            (
                Symbol::new(&env, "airdrop"),
                Symbol::new(&env, "campaign_created"),
            ),
            (
                campaign_id,
                campaign.merkle_root.clone(),
                campaign.token.clone(),
                total_allocation,
                deadline,
            ),
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
            (
                Symbol::new(&env, "airdrop"),
                Symbol::new(&env, "tokens_claimed"),
            ),
            (campaign_id, claimer, amount),
        );
    }

    /// Verify a merkle proof for eligibility.
    ///
    /// Leaf is: `sha256(address_strkey_bytes || amount_be_bytes)`. Each proof
    /// step concatenates the current 32-byte hash with the sibling element
    /// and rehashes. Rewritten for Soroban SDK 21.x: `Address::to_xdr` /
    /// `i128::to_xdr` / `Hash<32>::to_xdr` / `Hash<32> == BytesN<32>` are not
    /// available, so we serialize by `to_string()` + `i128::to_be_bytes()`
    /// and compare via byte arrays.
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

        // Build the leaf input: address strkey bytes || amount big-endian bytes.
        //
        // Bounds-check the strkey length (Stellar G-address strkeys are 56
        // chars / 56 bytes); panic if an unexpected address format appears
        // rather than silently producing a wrong leaf hash.
        let mut leaf_input: soroban_sdk::Bytes = soroban_sdk::Bytes::new(&env);
        let addr_str = address.to_string();
        let addr_len = addr_str.len() as usize;
        let mut addr_buf = [0u8; 64];
        if addr_len == 0 || addr_len > 64 {
            panic!("address strkey length out of expected range");
        }
        addr_str.copy_into_slice(&mut addr_buf[..addr_len]);
        let mut k = 0usize;
        while k < addr_len {
            leaf_input.push_back(addr_buf[k]);
            k += 1;
        }
        let amount_be: [u8; 16] = amount.to_be_bytes();
        let mut k = 0usize;
        while k < amount_be.len() {
            leaf_input.push_back(amount_be[k]);
            k += 1;
        }
        let mut current_hash = env.crypto().sha256(&leaf_input);

        // Walk the proof, hashing `cur_hash || sibling` (canonical
        // concatenated scheme, NOT byte-interleaved).
        let mut i = 0;
        while i < merkle_proof.len() {
            let proof_element = merkle_proof.get(i).unwrap();
            let mut combined_input: soroban_sdk::Bytes = soroban_sdk::Bytes::new(&env);
            // Soroban SDK 21.x: `Hash<32>` has no `copy_into_slice`; convert
            // to `BytesN<32>` (which does) via `From<Hash<32>>` first.
            let cur_bytes: BytesN<32> = BytesN::<32>::from(current_hash.clone());
            let mut cur_arr = [0u8; 32];
            cur_bytes.copy_into_slice(&mut cur_arr);
            let mut m = 0usize;
            while m < 32 {
                combined_input.push_back(cur_arr[m]);
                m += 1;
            }
            let mut elem_arr = [0u8; 32];
            proof_element.copy_into_slice(&mut elem_arr);
            m = 0usize;
            while m < 32 {
                combined_input.push_back(elem_arr[m]);
                m += 1;
            }
            current_hash = env.crypto().sha256(&combined_input);
            i += 1;
        }

        // Compare final hash with the campaign's merkle root via byte arrays
        // (Hash<32> and BytesN<32> don't implement PartialEq in SDK 21.x;
        // and Hash<32> has no copy_into_slice; convert via From<Hash<32>>).
        let cur_bytes: BytesN<32> = BytesN::<32>::from(current_hash);
        let mut cur = [0u8; 32];
        let mut root = [0u8; 32];
        cur_bytes.copy_into_slice(&mut cur);
        campaign.merkle_root.copy_into_slice(&mut root);
        cur == root
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
        token_client.transfer(&env.current_contract_address(), &admin, &unclaimed);

        // Emit UnclaimedReclaimed event
        env.events().publish(
            (
                Symbol::new(&env, "airdrop"),
                Symbol::new(&env, "unclaimed_reclaimed"),
            ),
            (campaign_id, unclaimed),
        );

        unclaimed
    }
}

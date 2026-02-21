#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec};

#[contracttype]
#[derive(Clone)]
pub struct DynamicNft {
    pub owner: Address,
    pub level: u32,
    pub rarity: u8,
    pub traits: String,
    pub metadata: String,
    pub minted_at: u64,
}

#[contracttype]
pub enum DataKey {
    Config(Address),
    DynamicNft(u32),
    NextNftId,
}

#[contract]
pub struct DynamicNftContract;

#[contractimpl]
impl DynamicNftContract {
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        if env.storage().persistent().has(&DataKey::Config(admin.clone())) {
            panic!("Already initialized for this admin");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Config(admin.clone()), &true);
        env.storage().persistent().set(&DataKey::NextNftId, &1u32);
    }

    pub fn mint(env: Env, minter: Address, owner: Address, metadata: String, traits: String) -> u32 {
        minter.require_auth();

        let next: u32 = env.storage().persistent().get(&DataKey::NextNftId).unwrap();
        let nft = DynamicNft {
            owner: owner.clone(),
            level: 1,
            rarity: 1,
            traits: traits.clone(),
            metadata: metadata.clone(),
            minted_at: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&DataKey::DynamicNft(next), &nft);
        env.storage().persistent().set(&DataKey::NextNftId, &(next + 1));
        env.events().publish((symbol_short!("mint"), owner, next), ());
        next
    }

    // evolve by milestone (admin or verifier in governance)
    pub fn evolve_milestone(env: Env, submitter: Address, token_id: u32, level_inc: u32, rarity_inc: u8, new_traits: Option<String>) {
        submitter.require_auth();
        let mut nft: DynamicNft = env.storage().persistent().get(&DataKey::DynamicNft(token_id)).unwrap();
        nft.level = nft.level.saturating_add(level_inc);
        nft.rarity = nft.rarity.saturating_add(rarity_inc);
        if let Some(t) = new_traits {
            nft.traits = t;
        }
        // append evolution note to metadata (simple history)
        let note = String::from_format(&env, "evolved_milestone:", &token_id.to_string());
        nft.metadata = String::from_format(&env, &nft.metadata, &note);
        env.storage().persistent().set(&DataKey::DynamicNft(token_id), &nft);
        env.events().publish((symbol_short!("evolve_milestone"), token_id), ());
    }

    // time-based evolution callable by anyone; checks elapsed time
    pub fn evolve_time(env: Env, caller: Address, token_id: u32, required_secs: u64) {
        caller.require_auth();
        let mut nft: DynamicNft = env.storage().persistent().get(&DataKey::DynamicNft(token_id)).unwrap();
        let now = env.ledger().timestamp();
        if now < nft.minted_at + required_secs {
            panic!("Not ready for time evolution");
        }
        nft.level = nft.level.saturating_add(1);
        nft.metadata = String::from_format(&env, &nft.metadata, &String::from_str(&env, "|time_evolved"));
        nft.minted_at = now; // reset timer for further evolutions
        env.storage().persistent().set(&DataKey::DynamicNft(token_id), &nft);
        env.events().publish((symbol_short!("evolve_time"), token_id), ());
    }

    pub fn downgrade(env: Env, submitter: Address, token_id: u32, level_dec: u32) {
        submitter.require_auth();
        let mut nft: DynamicNft = env.storage().persistent().get(&DataKey::DynamicNft(token_id)).unwrap();
        nft.level = nft.level.saturating_sub(level_dec);
        nft.metadata = String::from_format(&env, &nft.metadata, &String::from_str(&env, "|downgraded"));
        env.storage().persistent().set(&DataKey::DynamicNft(token_id), &nft);
        env.events().publish((symbol_short!("downgrade"), token_id), ());
    }

    // fuse two NFTs into a new one; owner must be same for both
    pub fn fuse(env: Env, submitter: Address, token_a: u32, token_b: u32) -> u32 {
        submitter.require_auth();
        let nft_a: DynamicNft = env.storage().persistent().get(&DataKey::DynamicNft(token_a)).unwrap();
        let nft_b: DynamicNft = env.storage().persistent().get(&DataKey::DynamicNft(token_b)).unwrap();
        if nft_a.owner != nft_b.owner {
            panic!("Owners must match to fuse");
        }
        // create fused NFT: summed level, higher rarity, combined traits
        let owner = nft_a.owner.clone();
        let fused_level = nft_a.level.saturating_add(nft_b.level);
        let fused_rarity = if nft_a.rarity > nft_b.rarity { nft_a.rarity } else { nft_b.rarity } + 1u8;
        let combined_traits = String::from_format(&env, &nft_a.traits, &String::from_format(&env, &String::from_str(&env, "+"), &nft_b.traits));
        let combined_metadata = String::from_format(&env, &nft_a.metadata, &nft_b.metadata);

        // simple burn: remove old entries
        env.storage().persistent().remove(&DataKey::DynamicNft(token_a));
        env.storage().persistent().remove(&DataKey::DynamicNft(token_b));

        let next: u32 = env.storage().persistent().get(&DataKey::NextNftId).unwrap();
        let nft = DynamicNft {
            owner: owner.clone(),
            level: fused_level,
            rarity: fused_rarity,
            traits: combined_traits,
            metadata: combined_metadata,
            minted_at: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&DataKey::DynamicNft(next), &nft);
        env.storage().persistent().set(&DataKey::NextNftId, &(next + 1));
        env.events().publish((symbol_short!("fuse"), owner, next), ());
        next
    }

    pub fn get_nft(env: Env, token_id: u32) -> Option<DynamicNft> {
        env.storage().persistent().get(&DataKey::DynamicNft(token_id))
    }
}

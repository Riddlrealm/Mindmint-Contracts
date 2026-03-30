#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, vec, Address, Bytes, Env, IntoVal,
    Vec,
};

// ─────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────

/// Whether the pack contents are predetermined or drawn at random.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackType {
    Fixed = 0,
    Blind = 1,
}

/// Status of a bundle.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleStatus {
    Active = 0,
    Closed = 1,
    Cancelled = 2,
}

/// A single NFT token reference: the NFT contract address + token id.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenRef {
    pub nft_contract: Address,
    pub token_id: u32,
}

/// Core bundle data stored on-chain.
#[contracttype]
#[derive(Clone, Debug)]
pub struct NftBundle {
    pub id: u64,
    pub creator: Address,
    pub pack_type: PackType,
    /// Remaining NFTs available for purchase (pool shrinks as packs are sold).
    pub nft_pool: Vec<TokenRef>,
    /// How many NFTs are delivered per pack purchase.
    pub nfts_per_pack: u32,
    /// Price in the payment token per pack.
    pub price: i128,
    /// Payment token address.
    pub payment_token: Address,
    /// Packs still available.
    pub packs_remaining: u32,
    pub status: BundleStatus,
}

#[contracttype]
pub enum DataKey {
    BundleCount,
    Bundle(u64),
}

// ─────────────────────────────────────────────
// Contract
// ─────────────────────────────────────────────

#[contract]
pub struct NftBundleContract;

#[contractimpl]
impl NftBundleContract {
    // ── create_bundle ────────────────────────────────────────────────────────

    /// Creator locks NFTs into the contract and defines the bundle.
    ///
    /// * `nft_pool`      – list of NFTs to lock; must be divisible by `nfts_per_pack`
    /// * `nfts_per_pack` – how many NFTs a buyer receives per pack
    /// * `price`         – payment token amount per pack
    /// * `payment_token` – token used for payment
    pub fn create_bundle(
        env: Env,
        creator: Address,
        pack_type: PackType,
        nft_pool: Vec<TokenRef>,
        nfts_per_pack: u32,
        price: i128,
        payment_token: Address,
    ) -> u64 {
        creator.require_auth();

        let pool_len = nft_pool.len();
        if pool_len == 0 {
            panic!("empty_pool");
        }
        if nfts_per_pack == 0 {
            panic!("zero_nfts_per_pack");
        }
        if pool_len % nfts_per_pack != 0 {
            panic!("pool_not_divisible_by_pack_size");
        }
        if price <= 0 {
            panic!("invalid_price");
        }

        let total_packs = pool_len / nfts_per_pack;

        // Transfer every NFT from creator into this contract.
        for token_ref in nft_pool.iter() {
            Self::transfer_nft(
                &env,
                &token_ref.nft_contract,
                &creator,
                &env.current_contract_address(),
                token_ref.token_id,
            );
        }

        let id = Self::next_bundle_id(&env);

        let bundle = NftBundle {
            id,
            creator: creator.clone(),
            pack_type,
            nft_pool,
            nfts_per_pack,
            price,
            payment_token,
            packs_remaining: total_packs,
            status: BundleStatus::Active,
        };

        env.storage().persistent().set(&DataKey::Bundle(id), &bundle);

        env.events().publish(
            (symbol_short!("BndlCrtd"), id),
            (creator, pack_type as u32, total_packs, price),
        );

        id
    }

    // ── purchase_pack ────────────────────────────────────────────────────────

    /// Buyer pays `price` and receives `nfts_per_pack` NFTs.
    ///
    /// Fixed packs take the first N NFTs from the pool (deterministic).
    /// Blind packs draw N NFTs pseudo-randomly using ledger entropy.
    pub fn purchase_pack(env: Env, buyer: Address, bundle_id: u64) -> Vec<TokenRef> {
        buyer.require_auth();

        let mut bundle = Self::require_active_bundle(&env, bundle_id);

        if bundle.packs_remaining == 0 {
            panic!("no_packs_remaining");
        }

        // Collect payment.
        token::Client::new(&env, &bundle.payment_token).transfer(
            &buyer,
            &env.current_contract_address(),
            &bundle.price,
        );

        let n = bundle.nfts_per_pack as usize;
        let received: Vec<TokenRef>;

        match bundle.pack_type {
            PackType::Fixed => {
                // Take the first N from the pool.
                received = Self::take_first_n(&env, &mut bundle.nft_pool, n);
            }
            PackType::Blind => {
                // Pseudo-random draw using ledger sequence + buyer address.
                received = Self::blind_draw(&env, &mut bundle.nft_pool, n, &buyer);
            }
        }

        // Transfer each drawn NFT to the buyer.
        for token_ref in received.iter() {
            Self::transfer_nft(
                &env,
                &token_ref.nft_contract,
                &env.current_contract_address(),
                &buyer,
                token_ref.token_id,
            );
        }

        bundle.packs_remaining -= 1;

        // Auto-close when the last pack is sold.
        if bundle.packs_remaining == 0 {
            bundle.status = BundleStatus::Closed;
            env.events().publish(
                (symbol_short!("BndlClsd"), bundle_id),
                (bundle_id,),
            );
        }

        env.storage().persistent().set(&DataKey::Bundle(bundle_id), &bundle);

        // Emit PackPurchased event.
        env.events().publish(
            (symbol_short!("PckPrchsd"), bundle_id),
            (buyer, bundle_id, received.clone()),
        );

        received
    }

    // ── cancel_bundle ────────────────────────────────────────────────────────

    /// Creator cancels an active bundle before it sells out.
    pub fn cancel_bundle(env: Env, creator: Address, bundle_id: u64) {
        creator.require_auth();

        let mut bundle: NftBundle = env
            .storage()
            .persistent()
            .get(&DataKey::Bundle(bundle_id))
            .unwrap_or_else(|| panic!("bundle_not_found"));

        if bundle.creator != creator {
            panic!("not_creator");
        }
        if bundle.status != BundleStatus::Active {
            panic!("bundle_not_active");
        }

        bundle.status = BundleStatus::Cancelled;
        env.storage().persistent().set(&DataKey::Bundle(bundle_id), &bundle);

        env.events().publish(
            (symbol_short!("BndlClsd"), bundle_id),
            (bundle_id,),
        );
    }

    // ── withdraw_unsold ──────────────────────────────────────────────────────

    /// Creator reclaims unsold NFTs after the bundle is closed or cancelled.
    pub fn withdraw_unsold(env: Env, creator: Address, bundle_id: u64) {
        creator.require_auth();

        let mut bundle: NftBundle = env
            .storage()
            .persistent()
            .get(&DataKey::Bundle(bundle_id))
            .unwrap_or_else(|| panic!("bundle_not_found"));

        if bundle.creator != creator {
            panic!("not_creator");
        }
        if bundle.status == BundleStatus::Active {
            panic!("bundle_still_active");
        }
        if bundle.nft_pool.is_empty() {
            panic!("nothing_to_withdraw");
        }

        // Return every remaining NFT to the creator.
        let remaining = bundle.nft_pool.clone();
        for token_ref in remaining.iter() {
            Self::transfer_nft(
                &env,
                &token_ref.nft_contract,
                &env.current_contract_address(),
                &creator,
                token_ref.token_id,
            );
        }

        bundle.nft_pool = Vec::new(&env);
        env.storage().persistent().set(&DataKey::Bundle(bundle_id), &bundle);
    }

    // ── withdraw_proceeds ────────────────────────────────────────────────────

    /// Creator withdraws accumulated payment token proceeds.
    pub fn withdraw_proceeds(env: Env, creator: Address, bundle_id: u64) {
        creator.require_auth();

        let bundle: NftBundle = env
            .storage()
            .persistent()
            .get(&DataKey::Bundle(bundle_id))
            .unwrap_or_else(|| panic!("bundle_not_found"));

        if bundle.creator != creator {
            panic!("not_creator");
        }

        let token_client = token::Client::new(&env, &bundle.payment_token);
        let balance = token_client.balance(&env.current_contract_address());
        if balance <= 0 {
            panic!("no_proceeds");
        }

        token_client.transfer(&env.current_contract_address(), &creator, &balance);
    }

    // ── get_bundle ───────────────────────────────────────────────────────────

    /// Returns pack type, price, packs remaining, and pool size.
    pub fn get_bundle(env: Env, bundle_id: u64) -> Option<NftBundle> {
        env.storage().persistent().get(&DataKey::Bundle(bundle_id))
    }

    // ─────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────

    fn next_bundle_id(env: &Env) -> u64 {
        let mut id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::BundleCount)
            .unwrap_or(0);
        id += 1;
        env.storage().instance().set(&DataKey::BundleCount, &id);
        id
    }

    fn require_active_bundle(env: &Env, bundle_id: u64) -> NftBundle {
        let bundle: NftBundle = env
            .storage()
            .persistent()
            .get(&DataKey::Bundle(bundle_id))
            .unwrap_or_else(|| panic!("bundle_not_found"));
        if bundle.status != BundleStatus::Active {
            panic!("bundle_not_active");
        }
        bundle
    }

    /// Remove and return the first `n` items from `pool`.
    fn take_first_n(env: &Env, pool: &mut Vec<TokenRef>, n: usize) -> Vec<TokenRef> {
        let mut out: Vec<TokenRef> = Vec::new(env);
        for _ in 0..n {
            out.push_back(pool.get(0).unwrap());
            pool.remove(0);
        }
        out
    }

    /// Pseudo-random draw of `n` distinct items from `pool`.
    ///
    /// Entropy: SHA-256( ledger_sequence_bytes ++ buyer_address_bytes )
    fn blind_draw(
        env: &Env,
        pool: &mut Vec<TokenRef>,
        n: usize,
        _buyer: &Address,
    ) -> Vec<TokenRef> {
        let mut out: Vec<TokenRef> = Vec::new(env);

        // Build entropy seed: ledger sequence (4 bytes) + timestamp (8 bytes).
        // Using on-chain ledger data as entropy per the issue spec.
        let seq = env.ledger().sequence();
        let ts = env.ledger().timestamp();
        let mut seed_bytes = Bytes::new(env);
        seed_bytes.extend_from_array(&seq.to_be_bytes());
        seed_bytes.extend_from_array(&ts.to_be_bytes());

        let hash = env.crypto().sha256(&seed_bytes);
        let hash_arr = hash.to_array();

        // Use successive 8-byte windows of the hash as random words.
        // Re-hash when we exhaust the 32 bytes.
        let mut rand_words: [u64; 4] = [
            u64::from_be_bytes(hash_arr[0..8].try_into().unwrap()),
            u64::from_be_bytes(hash_arr[8..16].try_into().unwrap()),
            u64::from_be_bytes(hash_arr[16..24].try_into().unwrap()),
            u64::from_be_bytes(hash_arr[24..32].try_into().unwrap()),
        ];
        let mut word_idx = 0usize;

        for _ in 0..n {
            let pool_len = pool.len() as u64;
            if pool_len == 0 {
                break;
            }

            // Refresh entropy when we've used all 4 words.
            if word_idx >= 4 {
                let mut rehash_input = Bytes::new(env);
                for w in rand_words.iter() {
                    rehash_input.extend_from_array(&w.to_be_bytes());
                }
                let new_hash = env.crypto().sha256(&rehash_input);
                let new_arr = new_hash.to_array();
                rand_words = [
                    u64::from_be_bytes(new_arr[0..8].try_into().unwrap()),
                    u64::from_be_bytes(new_arr[8..16].try_into().unwrap()),
                    u64::from_be_bytes(new_arr[16..24].try_into().unwrap()),
                    u64::from_be_bytes(new_arr[24..32].try_into().unwrap()),
                ];
                word_idx = 0;
            }

            let idx = (rand_words[word_idx] % pool_len) as u32;
            word_idx += 1;

            out.push_back(pool.get(idx).unwrap());
            pool.remove(idx);
        }

        out
    }

    /// Cross-contract NFT transfer helper.
    fn transfer_nft(
        env: &Env,
        nft_contract: &Address,
        from: &Address,
        to: &Address,
        token_id: u32,
    ) {
        let args: soroban_sdk::Vec<soroban_sdk::Val> = vec![
            env,
            from.clone().into_val(env),
            to.clone().into_val(env),
            token_id.into_val(env),
        ];
        env.invoke_contract::<()>(nft_contract, &soroban_sdk::Symbol::new(env, "transfer"), args);
    }
}

#[cfg(test)]
mod test;

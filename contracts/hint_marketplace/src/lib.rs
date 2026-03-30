#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, token, Address, Bytes, Env, Symbol, Vec,
};

// ──────────────────────────────────────────────────────────
// DATA STRUCTURES
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum HintQuality {
    Poor = 1,
    Fair = 2,
    Good = 3,
    Excellent = 4,
    Perfect = 5,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListingStatus {
    Active = 1,
    Sold = 2,
    Cancelled = 3,
    Expired = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hint {
    pub hint_id: u64,
    pub puzzle_id: u32,
    pub creator: Address,
    pub content_hash: Bytes,
    pub quality: HintQuality,
    pub created_at: u64,
    pub total_sales: u32,
    pub total_rating: u64,
    pub rating_count: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintListing {
    pub listing_id: u64,
    pub hint_id: u64,
    pub seller: Address,
    pub payment_token: Address,
    pub base_price: i128,
    pub current_price: i128,
    pub status: ListingStatus,
    pub created_time: u64,
    pub expiration_time: u64,
    pub creator: Address,
    pub royalty_bps: u32,
    pub quality: HintQuality,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintPack {
    pub pack_id: u64,
    pub name: Symbol,
    pub hint_ids: Vec<u64>,
    pub pack_price: i128,
    pub discount_bps: u32,
    pub creator: Address,
    pub created_at: u64,
    pub expiration_time: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rating {
    pub rater: Address,
    pub hint_id: u64,
    pub quality_rating: u32,
    pub helpfulness: u32,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketplaceConfig {
    pub admin: Address,
    pub fee_recipient: Address,
    pub fee_bps: u32,
    pub min_listing_duration: u64,
    pub max_listing_duration: u64,
    pub price_adjustment_factor: u32,
    pub min_quality_for_listing: HintQuality,
}

#[contracttype]
pub enum DataKey {
    Config,
    Hint(u64),
    HintCounter,
    Listing(u64),
    ListingCounter,
    Pack(u64),
    PackCounter,
    Rating(u64, Address),
    RatingsByHint(u64),
    ListingsByHint(u64),
    ListingsBySeller(Address),
    ListingsByPuzzle(u32),
    ActiveListings,
    PriceHistory(u64),
    DemandMetrics(u64),
    PacksByCreator(Address),
    ActivePacks,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemandMetrics {
    pub hint_id: u64,
    pub views: u32,
    pub purchases: u32,
    pub last_purchase_time: u64,
    pub average_time_to_sale: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketplaceError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    NotAuthorized = 3,
    HintNotFound = 4,
    ListingNotFound = 5,
    ListingNotActive = 6,
    ListingExpired = 7,
    InvalidPrice = 8,
    InvalidDuration = 9,
    InsufficientBalance = 10,
    InvalidQuality = 11,
    InvalidRating = 12,
    PackNotFound = 13,
    PackExpired = 14,
    DuplicateRating = 15,
}

// ──────────────────────────────────────────────────────────
// CONTRACT IMPLEMENTATION
// ──────────────────────────────────────────────────────────

#[contract]
pub struct HintMarketplace;

#[contractimpl]
impl HintMarketplace {
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_recipient: Address,
        fee_bps: u32,
        min_listing_duration: u64,
        max_listing_duration: u64,
        price_adjustment_factor: u32,
        min_quality_for_listing: HintQuality,
    ) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        if fee_bps > 10000 {
            panic!("Fee cannot exceed 100%");
        }
        if price_adjustment_factor > 10000 {
            panic!("Price adjustment factor cannot exceed 100%");
        }

        let config = MarketplaceConfig {
            admin,
            fee_recipient,
            fee_bps,
            min_listing_duration,
            max_listing_duration,
            price_adjustment_factor,
            min_quality_for_listing,
        };

        env.storage().instance().set(&DataKey::Config, &config);
        env.storage().instance().set(&DataKey::HintCounter, &0u64);
        env.storage().instance().set(&DataKey::ListingCounter, &0u64);
        env.storage().instance().set(&DataKey::PackCounter, &0u64);
    }

    pub fn update_config(
        env: Env,
        fee_recipient: Option<Address>,
        fee_bps: Option<u32>,
        min_listing_duration: Option<u64>,
        max_listing_duration: Option<u64>,
        price_adjustment_factor: Option<u32>,
        min_quality_for_listing: Option<HintQuality>,
    ) {
        let config: MarketplaceConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized");

        config.admin.require_auth();

        let mut new_config = config.clone();

        if let Some(recipient) = fee_recipient {
            new_config.fee_recipient = recipient;
        }
        if let Some(bps) = fee_bps {
            if bps > 10000 {
                panic!("Fee cannot exceed 100%");
            }
            new_config.fee_bps = bps;
        }
        if let Some(min) = min_listing_duration {
            new_config.min_listing_duration = min;
        }
        if let Some(max) = max_listing_duration {
            new_config.max_listing_duration = max;
        }
        if let Some(factor) = price_adjustment_factor {
            if factor > 10000 {
                panic!("Price adjustment factor cannot exceed 100%");
            }
            new_config.price_adjustment_factor = factor;
        }
        if let Some(quality) = min_quality_for_listing {
            new_config.min_quality_for_listing = quality;
        }

        env.storage().instance().set(&DataKey::Config, &new_config);
    }

    pub fn create_hint(
        env: Env,
        creator: Address,
        puzzle_id: u32,
        content_hash: Bytes,
        quality: HintQuality,
    ) -> u64 {
        creator.require_auth();

        let mut hint_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::HintCounter)
            .unwrap_or(0);
        hint_id += 1;
        env.storage().instance().set(&DataKey::HintCounter, &hint_id);

        let now = env.ledger().timestamp();

        let hint = Hint {
            hint_id,
            puzzle_id,
            creator: creator.clone(),
            content_hash,
            quality,
            created_at: now,
            total_sales: 0,
            total_rating: 0,
            rating_count: 0,
        };

        env.storage().instance().set(&DataKey::Hint(hint_id), &hint);

        let metrics = DemandMetrics {
            hint_id,
            views: 0,
            purchases: 0,
            last_purchase_time: 0,
            average_time_to_sale: 0,
        };
        env.storage()
            .instance()
            .set(&DataKey::DemandMetrics(hint_id), &metrics);

        hint_id
    }

    pub fn create_listing(
        env: Env,
        seller: Address,
        hint_id: u64,
        payment_token: Address,
        base_price: i128,
        duration: u64,
        royalty_bps: u32,
    ) -> u64 {
        seller.require_auth();

        let config: MarketplaceConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized");

        let hint: Hint = env
            .storage()
            .instance()
            .get(&DataKey::Hint(hint_id))
            .expect("Hint not found");

        if hint.quality < config.min_quality_for_listing {
            panic!("Hint quality below minimum requirement");
        }
        if base_price <= 0 {
            panic!("Price must be positive");
        }
        if royalty_bps > 10000 {
            panic!("Royalty cannot exceed 100%");
        }
        if duration < config.min_listing_duration || duration > config.max_listing_duration {
            panic!("Invalid listing duration");
        }

        let now = env.ledger().timestamp();
        let expiration_time = now + duration;

        let current_price = Self::calculate_dynamic_price(&env, hint_id, base_price);

        let mut listing_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ListingCounter)
            .unwrap_or(0);
        listing_id += 1;
        env.storage().instance().set(&DataKey::ListingCounter, &listing_id);

        let listing = HintListing {
            listing_id,
            hint_id,
            seller: seller.clone(),
            payment_token,
            base_price,
            current_price,
            status: ListingStatus::Active,
            created_time: now,
            expiration_time,
            creator: hint.creator.clone(),
            royalty_bps,
            quality: hint.quality,
        };

        env.storage()
            .instance()
            .set(&DataKey::Listing(listing_id), &listing);

        let mut hint_listings = Self::get_listings_by_hint_internal(&env, hint_id);
        hint_listings.push_back(listing_id);
        env.storage()
            .instance()
            .set(&DataKey::ListingsByHint(hint_id), &hint_listings);

        let mut seller_listings = Self::get_listings_by_seller_internal(&env, &seller);
        seller_listings.push_back(listing_id);
        env.storage()
            .instance()
            .set(&DataKey::ListingsBySeller(seller.clone()), &seller_listings);

        let mut puzzle_listings = Self::get_listings_by_puzzle_internal(&env, hint.puzzle_id);
        puzzle_listings.push_back(listing_id);
        env.storage()
            .instance()
            .set(&DataKey::ListingsByPuzzle(hint.puzzle_id), &puzzle_listings);

        let mut active_listings = Self::get_active_listings_internal(&env);
        active_listings.push_back(listing_id);
        env.storage()
            .instance()
            .set(&DataKey::ActiveListings, &active_listings);

        listing_id
    }

    pub fn buy(env: Env, buyer: Address, listing_id: u64) {
        buyer.require_auth();

        let mut listing: HintListing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(listing_id))
            .expect("Listing not found");

        if listing.status != ListingStatus::Active {
            panic!("Listing is not active");
        }

        let now = env.ledger().timestamp();
        if now > listing.expiration_time {
            listing.status = ListingStatus::Expired;
            env.storage()
                .instance()
                .set(&DataKey::Listing(listing_id), &listing);
            Self::remove_from_active_listings(&env, listing_id);
            panic!("Listing has expired");
        }

        if listing.seller == buyer {
            panic!("Cannot buy your own listing");
        }

        let config: MarketplaceConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized");

        let (seller_amount, fee_amount, royalty_amount) = Self::calculate_payouts(
            &env,
            listing.current_price,
            config.fee_bps,
            listing.royalty_bps,
        );

        let token_client = token::Client::new(&env, &listing.payment_token);
        token_client.transfer(&buyer, &env.current_contract_address(), &listing.current_price);
        token_client.transfer(&env.current_contract_address(), &listing.seller, &seller_amount);

        if fee_amount > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &config.fee_recipient,
                &fee_amount,
            );
        }

        if royalty_amount > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &listing.creator,
                &royalty_amount,
            );
        }

        let mut hint: Hint = env
            .storage()
            .instance()
            .get(&DataKey::Hint(listing.hint_id))
            .expect("Hint not found");
        hint.total_sales += 1;
        env.storage()
            .instance()
            .set(&DataKey::Hint(listing.hint_id), &hint);

        Self::update_demand_metrics(&env, listing.hint_id, listing.created_time, now);

        listing.status = ListingStatus::Sold;
        env.storage()
            .instance()
            .set(&DataKey::Listing(listing_id), &listing);

        Self::remove_from_active_listings(&env, listing_id);
        Self::record_price_history(&env, listing.hint_id, listing.current_price);
    }

    pub fn rate_hint(
        env: Env,
        rater: Address,
        hint_id: u64,
        quality_rating: u32,
        helpfulness: u32,
    ) {
        rater.require_auth();

        let mut hint: Hint = env
            .storage()
            .instance()
            .get(&DataKey::Hint(hint_id))
            .expect("Hint not found");

        if quality_rating < 1 || quality_rating > 5 || helpfulness < 1 || helpfulness > 5 {
            panic!("Rating must be between 1 and 5");
        }

        if env
            .storage()
            .instance()
            .has(&DataKey::Rating(hint_id, rater.clone()))
        {
            panic!("Already rated this hint");
        }

        let now = env.ledger().timestamp();

        let rating = Rating {
            rater: rater.clone(),
            hint_id,
            quality_rating,
            helpfulness,
            timestamp: now,
        };

        env.storage()
            .instance()
            .set(&DataKey::Rating(hint_id, rater.clone()), &rating);

        let mut ratings = Self::get_ratings_by_hint_internal(&env, hint_id);
        ratings.push_back(rater.clone());
        env.storage()
            .instance()
            .set(&DataKey::RatingsByHint(hint_id), &ratings);

        hint.total_rating += quality_rating as u64;
        hint.rating_count += 1;

        let avg_rating = hint.total_rating / hint.rating_count as u64;
        hint.quality = match avg_rating {
            1 => HintQuality::Poor,
            2 => HintQuality::Fair,
            3 => HintQuality::Good,
            4 => HintQuality::Excellent,
            5 => HintQuality::Perfect,
            _ => HintQuality::Good,
        };

        env.storage()
            .instance()
            .set(&DataKey::Hint(hint_id), &hint);
    }

    pub fn create_pack(
        env: Env,
        creator: Address,
        name: Symbol,
        hint_ids: Vec<u64>,
        pack_price: i128,
        discount_bps: u32,
        expiration_time: Option<u64>,
    ) -> u64 {
        creator.require_auth();

        if hint_ids.is_empty() {
            panic!("Pack must contain at least one hint");
        }
        if discount_bps > 10000 {
            panic!("Discount cannot exceed 100%");
        }

        for hint_id in hint_ids.iter() {
            let hint: Hint = env
                .storage()
                .instance()
                .get(&DataKey::Hint(hint_id))
                .expect("Hint not found");
            if hint.creator != creator {
                panic!("Not all hints belong to creator");
            }
        }

        let mut pack_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PackCounter)
            .unwrap_or(0);
        pack_id += 1;
        env.storage().instance().set(&DataKey::PackCounter, &pack_id);

        let now = env.ledger().timestamp();

        let pack = HintPack {
            pack_id,
            name,
            hint_ids: hint_ids.clone(),
            pack_price,
            discount_bps,
            creator: creator.clone(),
            created_at: now,
            expiration_time,
        };

        env.storage().instance().set(&DataKey::Pack(pack_id), &pack);

        let mut creator_packs = Self::get_packs_by_creator_internal(&env, &creator);
        creator_packs.push_back(pack_id);
        env.storage()
            .instance()
            .set(&DataKey::PacksByCreator(creator), &creator_packs);

        let mut active_packs = Self::get_active_packs_internal(&env);
        active_packs.push_back(pack_id);
        env.storage()
            .instance()
            .set(&DataKey::ActivePacks, &active_packs);

        pack_id
    }

    pub fn buy_pack(env: Env, buyer: Address, pack_id: u64, payment_token: Address) {
        buyer.require_auth();

        let pack: HintPack = env
            .storage()
            .instance()
            .get(&DataKey::Pack(pack_id))
            .expect("Pack not found");

        if let Some(exp_time) = pack.expiration_time {
            let now = env.ledger().timestamp();
            if now > exp_time {
                panic!("Pack has expired");
            }
        }

        let config: MarketplaceConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized");

        let (creator_amount, fee_amount, _royalty_amount) =
            Self::calculate_payouts(&env, pack.pack_price, config.fee_bps, 0);

        let token_client = token::Client::new(&env, &payment_token);
        token_client.transfer(&buyer, &env.current_contract_address(), &pack.pack_price);
        token_client.transfer(&env.current_contract_address(), &pack.creator, &creator_amount);

        if fee_amount > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &config.fee_recipient,
                &fee_amount,
            );
        }

        for hint_id in pack.hint_ids.iter() {
            let mut hint: Hint = env
                .storage()
                .instance()
                .get(&DataKey::Hint(hint_id))
                .expect("Hint not found");
            hint.total_sales += 1;
            env.storage()
                .instance()
                .set(&DataKey::Hint(hint_id), &hint);
        }
    }

    pub fn cancel_listing(env: Env, seller: Address, listing_id: u64) {
        seller.require_auth();

        let mut listing: HintListing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(listing_id))
            .expect("Listing not found");

        if listing.seller != seller {
            panic!("Not the listing seller");
        }
        if listing.status != ListingStatus::Active {
            panic!("Listing is not active");
        }

        listing.status = ListingStatus::Cancelled;
        env.storage()
            .instance()
            .set(&DataKey::Listing(listing_id), &listing);

        Self::remove_from_active_listings(&env, listing_id);
    }

    pub fn expire_listings(env: Env, listing_ids: Vec<u64>) {
        let now = env.ledger().timestamp();

        for listing_id in listing_ids.iter() {
            if let Some(mut listing) = env
                .storage()
                .instance()
                .get::<DataKey, HintListing>(&DataKey::Listing(listing_id))
            {
                if listing.status == ListingStatus::Active && now > listing.expiration_time {
                    listing.status = ListingStatus::Expired;
                    env.storage()
                        .instance()
                        .set(&DataKey::Listing(listing_id), &listing);
                    Self::remove_from_active_listings(&env, listing_id);
                }
            }
        }
    }

    // ──────────────────────────────────────────────────────────
    // PRIVATE HELPER FUNCTIONS
    // ──────────────────────────────────────────────────────────

    fn calculate_dynamic_price(env: &Env, hint_id: u64, base_price: i128) -> i128 {
        let config: MarketplaceConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized");

        let metrics: DemandMetrics = env
            .storage()
            .instance()
            .get(&DataKey::DemandMetrics(hint_id))
            .unwrap_or(DemandMetrics {
                hint_id,
                views: 0,
                purchases: 0,
                last_purchase_time: 0,
                average_time_to_sale: 0,
            });

        let hint: Hint = env
            .storage()
            .instance()
            .get(&DataKey::Hint(hint_id))
            .expect("Hint not found");

        let quality_multiplier = match hint.quality {
            HintQuality::Poor => 5000,
            HintQuality::Fair => 7500,
            HintQuality::Good => 10000,
            HintQuality::Excellent => 12500,
            HintQuality::Perfect => 15000,
        };

        let quality_adjusted_price = (base_price * quality_multiplier as i128) / 10000;

        let demand_factor = if metrics.purchases > 0 {
            let purchase_rate = (metrics.purchases as i128 * 10000)
                / (metrics.views as i128 + metrics.purchases as i128).max(1);
            10000 + (purchase_rate * config.price_adjustment_factor as i128) / 10000
        } else {
            10000
        };

        let final_price = (quality_adjusted_price * demand_factor) / 10000;

        let min_price = base_price / 2;
        let max_price = base_price * 3;
        final_price.max(min_price).min(max_price)
    }

    fn calculate_payouts(
        _env: &Env,
        price: i128,
        fee_bps: u32,
        royalty_bps: u32,
    ) -> (i128, i128, i128) {
        let fee_amount = (price * fee_bps as i128) / 10000;
        let royalty_amount = (price * royalty_bps as i128) / 10000;
        let seller_amount = price - fee_amount - royalty_amount;
        (seller_amount, fee_amount, royalty_amount)
    }

    fn update_demand_metrics(env: &Env, hint_id: u64, listing_created: u64, purchase_time: u64) {
        let mut metrics: DemandMetrics = env
            .storage()
            .instance()
            .get(&DataKey::DemandMetrics(hint_id))
            .unwrap_or(DemandMetrics {
                hint_id,
                views: 0,
                purchases: 0,
                last_purchase_time: 0,
                average_time_to_sale: 0,
            });

        metrics.purchases += 1;
        metrics.last_purchase_time = purchase_time;

        let time_to_sale = purchase_time - listing_created;
        if metrics.purchases == 1 {
            metrics.average_time_to_sale = time_to_sale;
        } else {
            metrics.average_time_to_sale =
                (metrics.average_time_to_sale * (metrics.purchases - 1) as u64 + time_to_sale)
                    / metrics.purchases as u64;
        }

        env.storage()
            .instance()
            .set(&DataKey::DemandMetrics(hint_id), &metrics);
    }

    fn record_price_history(env: &Env, hint_id: u64, price: i128) {
        let mut history: Vec<i128> = env
            .storage()
            .instance()
            .get(&DataKey::PriceHistory(hint_id))
            .unwrap_or(Vec::new(env));

        history.push_back(price);

        if history.len() > 100 {
            let mut new_history = Vec::new(env);
            let start_index = history.len() - 100;
            for i in start_index..history.len() {
                new_history.push_back(history.get(i).unwrap());
            }
            history = new_history;
        }

        env.storage()
            .instance()
            .set(&DataKey::PriceHistory(hint_id), &history);
    }

    fn remove_from_active_listings(env: &Env, listing_id: u64) {
        let mut active_listings = Self::get_active_listings_internal(env);
        if let Some(index) = active_listings.first_index_of(listing_id) {
            active_listings.remove(index);
            env.storage()
                .instance()
                .set(&DataKey::ActiveListings, &active_listings);
        }
    }

    // ──────────────────────────────────────────────────────────
    // PRIVATE INTERNAL GETTERS (used by other contract methods)
    // ──────────────────────────────────────────────────────────

    fn get_listings_by_hint_internal(env: &Env, hint_id: u64) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::ListingsByHint(hint_id))
            .unwrap_or(Vec::new(env))
    }

    fn get_listings_by_seller_internal(env: &Env, seller: &Address) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::ListingsBySeller(seller.clone()))
            .unwrap_or(Vec::new(env))
    }

    fn get_listings_by_puzzle_internal(env: &Env, puzzle_id: u32) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::ListingsByPuzzle(puzzle_id))
            .unwrap_or(Vec::new(env))
    }

    fn get_active_listings_internal(env: &Env) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::ActiveListings)
            .unwrap_or(Vec::new(env))
    }

    fn get_ratings_by_hint_internal(env: &Env, hint_id: u64) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::RatingsByHint(hint_id))
            .unwrap_or(Vec::new(env))
    }

    fn get_packs_by_creator_internal(env: &Env, creator: &Address) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::PacksByCreator(creator.clone()))
            .unwrap_or(Vec::new(env))
    }

    fn get_active_packs_internal(env: &Env) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::ActivePacks)
            .unwrap_or(Vec::new(env))
    }

    // ──────────────────────────────────────────────────────────
    // PUBLIC GETTER FUNCTIONS (ABI-exposed, owned types only)
    // ──────────────────────────────────────────────────────────

    pub fn get_hint(env: Env, hint_id: u64) -> Option<Hint> {
        env.storage().instance().get(&DataKey::Hint(hint_id))
    }

    pub fn get_listing(env: Env, listing_id: u64) -> Option<HintListing> {
        env.storage().instance().get(&DataKey::Listing(listing_id))
    }

    pub fn get_pack(env: Env, pack_id: u64) -> Option<HintPack> {
        env.storage().instance().get(&DataKey::Pack(pack_id))
    }

    pub fn get_rating(env: Env, hint_id: u64, rater: Address) -> Option<Rating> {
        env.storage().instance().get(&DataKey::Rating(hint_id, rater))
    }

    pub fn get_listings_by_hint(env: Env, hint_id: u64) -> Vec<u64> {
        Self::get_listings_by_hint_internal(&env, hint_id)
    }

    pub fn get_listings_by_seller(env: Env, seller: Address) -> Vec<u64> {
        Self::get_listings_by_seller_internal(&env, &seller)
    }

    pub fn get_listings_by_puzzle(env: Env, puzzle_id: u32) -> Vec<u64> {
        Self::get_listings_by_puzzle_internal(&env, puzzle_id)
    }

    pub fn get_active_listings(env: Env) -> Vec<u64> {
        Self::get_active_listings_internal(&env)
    }

    pub fn get_ratings_by_hint(env: Env, hint_id: u64) -> Vec<Address> {
        Self::get_ratings_by_hint_internal(&env, hint_id)
    }

    pub fn get_price_history(env: Env, hint_id: u64) -> Vec<i128> {
        env.storage()
            .instance()
            .get(&DataKey::PriceHistory(hint_id))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_demand_metrics(env: Env, hint_id: u64) -> Option<DemandMetrics> {
        env.storage().instance().get(&DataKey::DemandMetrics(hint_id))
    }

    pub fn get_packs_by_creator(env: Env, creator: Address) -> Vec<u64> {
        Self::get_packs_by_creator_internal(&env, &creator)
    }

    pub fn get_active_packs(env: Env) -> Vec<u64> {
        Self::get_active_packs_internal(&env)
    }

    pub fn get_config(env: Env) -> MarketplaceConfig {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized")
    }
}

#[cfg(test)]
mod test;
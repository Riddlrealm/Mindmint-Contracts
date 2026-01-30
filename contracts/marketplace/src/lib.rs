#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, token, Address, Env, IntoVal, Symbol, Vec, Map,
};

// ──────────────────────────────────────────────────────────
// DATA STRUCTURES
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetType {
    NFT = 1,
    Item = 2,
    Hint = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    pub asset_type: AssetType,
    pub contract: Address,
    pub token_id: u32,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListingStatus {
    Active = 1,
    Sold = 2,
    Cancelled = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Listing {
    pub listing_id: u64,
    pub seller: Address,
    pub asset: Asset,
    pub payment_token: Address,
    pub price: i128,
    pub status: ListingStatus,
    pub created_time: u64,
    pub creator: Option<Address>, // For royalty payments
    pub royalty_bps: u32, // Royalty in basis points (10000 = 100%)
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfferStatus {
    Open = 1,
    Accepted = 2,
    Rejected = 3,
    Cancelled = 4,
    Countered = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Offer {
    pub offer_id: u64,
    pub listing_id: u64,
    pub buyer: Address,
    pub price: i128,
    pub status: OfferStatus,
    pub created_time: u64,
    pub expiration_time: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CounterOffer {
    pub counter_offer_id: u64,
    pub offer_id: u64,
    pub seller: Address,
    pub price: i128,
    pub created_time: u64,
    pub expiration_time: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketplaceConfig {
    pub admin: Address,
    pub fee_recipient: Address,
    pub fee_bps: u32, // Marketplace fee in basis points (10000 = 100%)
    pub min_listing_duration: u64,
    pub max_listing_duration: u64,
}

#[contracttype]
pub enum DataKey {
    Config,                          // MarketplaceConfig
    Listing(u64),                    // Listing
    ListingCount,                    // u64
    Offer(u64),                      // Offer
    CounterOffer(u64),               // CounterOffer
    OfferCount,                      // u64
    CounterOfferCount,               // u64
    OffersByListing(u64),            // Vec<u64> - offer IDs for a listing
    CounterOffersByOffer(u64),       // Vec<u64> - counter offer IDs for an offer
    ListingsBySeller(Address),       // Vec<u64> - listing IDs by seller
    ListingsByAsset(Address, u32),   // Vec<u64> - listing IDs by asset
    ActiveListings,                  // Vec<u64> - all active listings
    PriceHistory(Address, u32),      // Vec<i128> - price history for an asset
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketplaceError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    NotAuthorized = 3,
    ListingNotFound = 4,
    ListingNotActive = 5,
    OfferNotFound = 6,
    InvalidPrice = 7,
    InvalidDuration = 8,
    AssetNotOwned = 9,
    InsufficientBalance = 10,
    OfferExpired = 11,
    InvalidAssetType = 12,
}

// ──────────────────────────────────────────────────────────
// CONTRACT IMPLEMENTATION
// ──────────────────────────────────────────────────────────

#[contract]
pub struct MarketplaceContract;

#[contractimpl]
impl MarketplaceContract {
    /// Initialize the marketplace contract
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_recipient: Address,
        fee_bps: u32,
        min_listing_duration: u64,
        max_listing_duration: u64,
    ) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }

        if fee_bps > 10000 {
            panic!("Fee cannot exceed 100%");
        }

        let config = MarketplaceConfig {
            admin,
            fee_recipient,
            fee_bps,
            min_listing_duration,
            max_listing_duration,
        };

        env.storage().instance().set(&DataKey::Config, &config);
        env.storage().instance().set(&DataKey::ListingCount, &0u64);
        env.storage().instance().set(&DataKey::OfferCount, &0u64);
        env.storage().instance().set(&DataKey::CounterOfferCount, &0u64);
    }

    /// Update marketplace configuration (admin only)
    pub fn update_config(
        env: Env,
        fee_recipient: Option<Address>,
        fee_bps: Option<u32>,
        min_listing_duration: Option<u64>,
        max_listing_duration: Option<u64>,
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

        env.storage().instance().set(&DataKey::Config, &new_config);
    }

    /// Create a new listing for an NFT or item
    pub fn create_listing(
        env: Env,
        seller: Address,
        asset: Asset,
        payment_token: Address,
        price: i128,
        creator: Option<Address>,
        royalty_bps: u32,
    ) -> u64 {
        seller.require_auth();

        if price <= 0 {
            panic!("Price must be positive");
        }

        if royalty_bps > 10000 {
            panic!("Royalty cannot exceed 100%");
        }

        // Verify seller owns the asset
        Self::verify_asset_ownership(&env, &seller, &asset);

        // Transfer asset to contract (escrow)
        Self::transfer_asset_to_contract(&env, &seller, &asset);

        // Generate listing ID
        let mut listing_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ListingCount)
            .unwrap_or(0);
        listing_id += 1;
        env.storage().instance().set(&DataKey::ListingCount, &listing_id);

        // Create listing
        let listing = Listing {
            listing_id,
            seller: seller.clone(),
            asset: asset.clone(),
            payment_token,
            price,
            status: ListingStatus::Active,
            created_time: env.ledger().timestamp(),
            creator,
            royalty_bps,
        };

        // Save listing
        env.storage()
            .instance()
            .set(&DataKey::Listing(listing_id), &listing);

        // Update indexes
        let mut seller_listings = Self::get_listings_by_seller(&env, &seller);
        seller_listings.push_back(listing_id);
        env.storage()
            .instance()
            .set(&DataKey::ListingsBySeller(seller.clone()), &seller_listings);

        let mut asset_listings = Self::get_listings_by_asset(&env, &asset.contract, &asset.token_id);
        asset_listings.push_back(listing_id);
        env.storage()
            .instance()
            .set(&DataKey::ListingsByAsset(asset.contract.clone(), asset.token_id), &asset_listings);

        let mut active_listings = Self::get_active_listings(&env);
        active_listings.push_back(listing_id);
        env.storage()
            .instance()
            .set(&DataKey::ActiveListings, &active_listings);

        listing_id
    }

    /// Buy a listed item directly
    pub fn buy(env: Env, buyer: Address, listing_id: u64) {
        buyer.require_auth();

        let mut listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(listing_id))
            .expect("Listing not found");

        if listing.status != ListingStatus::Active {
            panic!("Listing is not active");
        }

        if listing.seller == buyer {
            panic!("Cannot buy your own listing");
        }

        let config: MarketplaceConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized");

        // Calculate fees and royalties
        let (seller_amount, fee_amount, royalty_amount) = Self::calculate_payouts(
            &env,
            listing.price,
            config.fee_bps,
            listing.royalty_bps,
        );

        // Transfer payment from buyer to contract
        let token_client = token::Client::new(&env, &listing.payment_token);
        token_client.transfer(&buyer, &env.current_contract_address(), &listing.price);

        // Distribute payments
        // 1. Pay seller (after fees and royalties)
        token_client.transfer(&env.current_contract_address(), &listing.seller, &seller_amount);

        // 2. Pay marketplace fee
        if fee_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &config.fee_recipient, &fee_amount);
        }

        // 3. Pay royalty to creator
        if royalty_amount > 0 {
            if let Some(creator) = listing.creator.clone() {
                token_client.transfer(&env.current_contract_address(), &creator, &royalty_amount);
            }
        }

        // Transfer asset from contract to buyer
        Self::transfer_asset_from_contract(&env, &buyer, &listing.asset);

        // Update listing status
        listing.status = ListingStatus::Sold;
        env.storage()
            .instance()
            .set(&DataKey::Listing(listing_id), &listing);

        // Remove from active listings
        Self::remove_from_active_listings(&env, listing_id);

        // Record price in history
        Self::record_price_history(&env, &listing.asset.contract, &listing.asset.token_id, listing.price);
    }

    /// Create an offer on a listing
    pub fn create_offer(
        env: Env,
        buyer: Address,
        listing_id: u64,
        price: i128,
        expiration_time: Option<u64>,
    ) -> u64 {
        buyer.require_auth();

        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(listing_id))
            .expect("Listing not found");

        if listing.status != ListingStatus::Active {
            panic!("Listing is not active");
        }

        if listing.seller == buyer {
            panic!("Cannot offer on your own listing");
        }

        if price <= 0 {
            panic!("Price must be positive");
        }

        // Check expiration
        if let Some(exp_time) = expiration_time {
            if exp_time <= env.ledger().timestamp() {
                panic!("Expiration time must be in the future");
            }
        }

        // Generate offer ID
        let mut offer_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::OfferCount)
            .unwrap_or(0);
        offer_id += 1;
        env.storage().instance().set(&DataKey::OfferCount, &offer_id);

        // Create offer
        let offer = Offer {
            offer_id,
            listing_id,
            buyer: buyer.clone(),
            price,
            status: OfferStatus::Open,
            created_time: env.ledger().timestamp(),
            expiration_time,
        };

        // Save offer
        env.storage()
            .instance()
            .set(&DataKey::Offer(offer_id), &offer);

        // Update indexes
        let mut listing_offers = Self::get_offers_by_listing(&env, listing_id);
        listing_offers.push_back(offer_id);
        env.storage()
            .instance()
            .set(&DataKey::OffersByListing(listing_id), &listing_offers);

        // Transfer payment to contract (escrow)
        let token_client = token::Client::new(&env, &listing.payment_token);
        token_client.transfer(&buyer, &env.current_contract_address(), &price);

        offer_id
    }

    /// Accept an offer
    pub fn accept_offer(env: Env, seller: Address, offer_id: u64) {
        seller.require_auth();

        let mut offer: Offer = env
            .storage()
            .instance()
            .get(&DataKey::Offer(offer_id))
            .expect("Offer not found");

        if offer.status != OfferStatus::Open {
            panic!("Offer is not open");
        }

        // Check expiration
        if let Some(exp_time) = offer.expiration_time {
            if env.ledger().timestamp() > exp_time {
                panic!("Offer has expired");
            }
        }

        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(offer.listing_id))
            .expect("Listing not found");

        if listing.seller != seller {
            panic!("Not the listing seller");
        }

        if listing.status != ListingStatus::Active {
            panic!("Listing is not active");
        }

        let config: MarketplaceConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized");

        // Calculate fees and royalties
        let (seller_amount, fee_amount, royalty_amount) = Self::calculate_payouts(
            &env,
            offer.price,
            config.fee_bps,
            listing.royalty_bps,
        );

        let token_client = token::Client::new(&env, &listing.payment_token);

        // Distribute payments
        // 1. Pay seller (after fees and royalties)
        token_client.transfer(&env.current_contract_address(), &seller, &seller_amount);

        // 2. Pay marketplace fee
        if fee_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &config.fee_recipient, &fee_amount);
        }

        // 3. Pay royalty to creator
        if royalty_amount > 0 {
            if let Some(creator) = listing.creator.clone() {
                token_client.transfer(&env.current_contract_address(), &creator, &royalty_amount);
            }
        }

        // Transfer asset from contract to buyer
        Self::transfer_asset_from_contract(&env, &offer.buyer, &listing.asset);

        // Update offer status
        offer.status = OfferStatus::Accepted;
        env.storage()
            .instance()
            .set(&DataKey::Offer(offer_id), &offer);

        // Update listing status
        let mut listing = listing;
        listing.status = ListingStatus::Sold;
        env.storage()
            .instance()
            .set(&DataKey::Listing(offer.listing_id), &listing);

        // Remove from active listings
        Self::remove_from_active_listings(&env, offer.listing_id);

        // Refund other offers on this listing
        Self::refund_other_offers(&env, offer.listing_id, offer_id);

        // Record price in history
        Self::record_price_history(&env, &listing.asset.contract, &listing.asset.token_id, offer.price);
    }

    /// Reject an offer (refund buyer)
    pub fn reject_offer(env: Env, seller: Address, offer_id: u64) {
        seller.require_auth();

        let mut offer: Offer = env
            .storage()
            .instance()
            .get(&DataKey::Offer(offer_id))
            .expect("Offer not found");

        if offer.status != OfferStatus::Open {
            panic!("Offer is not open");
        }

        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(offer.listing_id))
            .expect("Listing not found");

        if listing.seller != seller {
            panic!("Not the listing seller");
        }

        // Refund buyer
        let token_client = token::Client::new(&env, &listing.payment_token);
        token_client.transfer(&env.current_contract_address(), &offer.buyer, &offer.price);

        // Update offer status
        offer.status = OfferStatus::Rejected;
        env.storage()
            .instance()
            .set(&DataKey::Offer(offer_id), &offer);
    }

    /// Create a counter-offer
    pub fn create_counter_offer(
        env: Env,
        seller: Address,
        offer_id: u64,
        price: i128,
        expiration_time: Option<u64>,
    ) -> u64 {
        seller.require_auth();

        let offer: Offer = env
            .storage()
            .instance()
            .get(&DataKey::Offer(offer_id))
            .expect("Offer not found");

        if offer.status != OfferStatus::Open {
            panic!("Offer is not open");
        }

        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(offer.listing_id))
            .expect("Listing not found");

        if listing.seller != seller {
            panic!("Not the listing seller");
        }

        if price <= 0 {
            panic!("Price must be positive");
        }

        // Check expiration
        if let Some(exp_time) = expiration_time {
            if exp_time <= env.ledger().timestamp() {
                panic!("Expiration time must be in the future");
            }
        }

        // Generate counter offer ID
        let mut counter_offer_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CounterOfferCount)
            .unwrap_or(0);
        counter_offer_id += 1;
        env.storage().instance().set(&DataKey::CounterOfferCount, &counter_offer_id);

        // Create counter offer
        let counter_offer = CounterOffer {
            counter_offer_id,
            offer_id,
            seller: seller.clone(),
            price,
            created_time: env.ledger().timestamp(),
            expiration_time,
        };

        // Save counter offer
        env.storage()
            .instance()
            .set(&DataKey::CounterOffer(counter_offer_id), &counter_offer);

        // Update indexes
        let mut offer_counters = Self::get_counter_offers_by_offer(&env, offer_id);
        offer_counters.push_back(counter_offer_id);
        env.storage()
            .instance()
            .set(&DataKey::CounterOffersByOffer(offer_id), &offer_counters);

        // Mark original offer as countered
        let mut offer = offer;
        offer.status = OfferStatus::Countered;
        env.storage()
            .instance()
            .set(&DataKey::Offer(offer_id), &offer);

        counter_offer_id
    }

    /// Accept a counter-offer
    pub fn accept_counter_offer(env: Env, buyer: Address, counter_offer_id: u64) {
        buyer.require_auth();

        let counter_offer: CounterOffer = env
            .storage()
            .instance()
            .get(&DataKey::CounterOffer(counter_offer_id))
            .expect("Counter offer not found");

        let offer: Offer = env
            .storage()
            .instance()
            .get(&DataKey::Offer(counter_offer.offer_id))
            .expect("Offer not found");

        if offer.buyer != buyer {
            panic!("Not the offer buyer");
        }

        // Check expiration
        if let Some(exp_time) = counter_offer.expiration_time {
            if env.ledger().timestamp() > exp_time {
                panic!("Counter offer has expired");
            }
        }

        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(offer.listing_id))
            .expect("Listing not found");

        let config: MarketplaceConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized");

        let token_client = token::Client::new(&env, &listing.payment_token);

        // Refund original offer amount
        token_client.transfer(&env.current_contract_address(), &buyer, &offer.price);

        // Take new payment amount
        let price_difference = counter_offer.price - offer.price;
        if price_difference > 0 {
            token_client.transfer(&buyer, &env.current_contract_address(), &price_difference);
        }

        // Calculate fees and royalties
        let (seller_amount, fee_amount, royalty_amount) = Self::calculate_payouts(
            &env,
            counter_offer.price,
            config.fee_bps,
            listing.royalty_bps,
        );

        // Distribute payments
        token_client.transfer(&env.current_contract_address(), &counter_offer.seller, &seller_amount);

        if fee_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &config.fee_recipient, &fee_amount);
        }

        if royalty_amount > 0 {
            if let Some(creator) = listing.creator.clone() {
                token_client.transfer(&env.current_contract_address(), &creator, &royalty_amount);
            }
        }

        // Transfer asset from contract to buyer
        Self::transfer_asset_from_contract(&env, &buyer, &listing.asset);

        // Update offer status
        let mut offer = offer;
        offer.status = OfferStatus::Accepted;
        env.storage()
            .instance()
            .set(&DataKey::Offer(counter_offer.offer_id), &offer);

        // Update listing status
        let mut listing = listing;
        listing.status = ListingStatus::Sold;
        env.storage()
            .instance()
            .set(&DataKey::Listing(offer.listing_id), &listing);

        // Remove from active listings
        Self::remove_from_active_listings(&env, offer.listing_id);

        // Refund other offers on this listing
        Self::refund_other_offers(&env, offer.listing_id, counter_offer.offer_id);

        // Record price in history
        Self::record_price_history(&env, &listing.asset.contract, &listing.asset.token_id, counter_offer.price);
    }

    /// Cancel a listing
    pub fn cancel_listing(env: Env, seller: Address, listing_id: u64) {
        seller.require_auth();

        let mut listing: Listing = env
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

        // Return asset to seller
        Self::transfer_asset_from_contract(&env, &seller, &listing.asset);

        // Refund all offers on this listing
        Self::refund_all_offers(&env, listing_id);

        // Update listing status
        listing.status = ListingStatus::Cancelled;
        env.storage()
            .instance()
            .set(&DataKey::Listing(listing_id), &listing);

        // Remove from active listings
        Self::remove_from_active_listings(&env, listing_id);
    }

    /// Cancel an offer
    pub fn cancel_offer(env: Env, buyer: Address, offer_id: u64) {
        buyer.require_auth();

        let mut offer: Offer = env
            .storage()
            .instance()
            .get(&DataKey::Offer(offer_id))
            .expect("Offer not found");

        if offer.buyer != buyer {
            panic!("Not the offer buyer");
        }

        if offer.status != OfferStatus::Open {
            panic!("Offer is not open");
        }

        // Refund buyer
        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(offer.listing_id))
            .expect("Listing not found");

        let token_client = token::Client::new(&env, &listing.payment_token);
        token_client.transfer(&env.current_contract_address(), &buyer, &offer.price);

        // Update offer status
        offer.status = OfferStatus::Cancelled;
        env.storage()
            .instance()
            .set(&DataKey::Offer(offer_id), &offer);
    }

    // ──────────────────────────────────────────────────────────
    // HELPER FUNCTIONS
    // ──────────────────────────────────────────────────────────

    /// Verify asset ownership
    fn verify_asset_ownership(env: &Env, owner: &Address, asset: &Asset) {
        // Invoke the NFT contract's owner_of function
        let owner_of_args = (asset.token_id,).into_val(env);
        let result: Address = env
            .invoke_contract(
                &asset.contract,
                &Symbol::new(env, "owner_of"),
                owner_of_args,
            );

        if result != *owner {
            panic!("Asset not owned by seller");
        }
    }

    /// Transfer asset to contract (escrow)
    fn transfer_asset_to_contract(env: &Env, from: &Address, asset: &Asset) {
        let transfer_args = (from.clone(), env.current_contract_address(), asset.token_id).into_val(env);
        env.invoke_contract::<()>(
            &asset.contract,
            &Symbol::new(env, "transfer"),
            transfer_args,
        );
    }

    /// Transfer asset from contract to buyer
    fn transfer_asset_from_contract(env: &Env, to: &Address, asset: &Asset) {
        let transfer_args = (env.current_contract_address(), to.clone(), asset.token_id).into_val(env);
        env.invoke_contract::<()>(
            &asset.contract,
            &Symbol::new(env, "transfer"),
            transfer_args,
        );
    }

    /// Calculate payouts (seller amount, fee amount, royalty amount)
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

    /// Record price in history for price discovery
    fn record_price_history(env: &Env, contract: &Address, token_id: &u32, price: i128) {
        let mut history: Vec<i128> = env
            .storage()
            .instance()
            .get(&DataKey::PriceHistory(contract.clone(), *token_id))
            .unwrap_or(Vec::new(env));

        history.push_back(price);

        // Keep only last 100 prices
        if history.len() > 100 {
            // Create a new Vec with only the last 100 elements
            let mut new_history = Vec::new(env);
            let start_index = history.len() - 100;
            for i in start_index..history.len() {
                new_history.push_back(*history.get(i).unwrap());
            }
            history = new_history;
        }

        env.storage()
            .instance()
            .set(&DataKey::PriceHistory(contract.clone(), *token_id), &history);
    }

    /// Remove listing from active listings
    fn remove_from_active_listings(env: &Env, listing_id: u64) {
        let mut active_listings = Self::get_active_listings(env);
        if let Some(index) = active_listings.first_index_of(listing_id) {
            active_listings.remove(index);
            env.storage()
                .instance()
                .set(&DataKey::ActiveListings, &active_listings);
        }
    }

    /// Refund all offers on a listing
    fn refund_all_offers(env: &Env, listing_id: u64) {
        let offers = Self::get_offers_by_listing(env, listing_id);
        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(listing_id))
            .expect("Listing not found");

        let token_client = token::Client::new(env, &listing.payment_token);

        for offer_id in offers.iter() {
            if let Some(mut offer) = env.storage().instance().get::<DataKey, Offer>(&DataKey::Offer(offer_id)) {
                if offer.status == OfferStatus::Open {
                    token_client.transfer(&env.current_contract_address(), &offer.buyer, &offer.price);
                    offer.status = OfferStatus::Cancelled;
                    env.storage()
                        .instance()
                        .set(&DataKey::Offer(offer_id), &offer);
                }
            }
        }
    }

    /// Refund other offers (except the accepted one)
    fn refund_other_offers(env: &Env, listing_id: u64, accepted_offer_id: u64) {
        let offers = Self::get_offers_by_listing(env, listing_id);
        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(listing_id))
            .expect("Listing not found");

        let token_client = token::Client::new(env, &listing.payment_token);

        for offer_id in offers.iter() {
            if offer_id != accepted_offer_id {
                if let Some(mut offer) = env.storage().instance().get::<DataKey, Offer>(&DataKey::Offer(offer_id)) {
                    if offer.status == OfferStatus::Open {
                        token_client.transfer(&env.current_contract_address(), &offer.buyer, &offer.price);
                        offer.status = OfferStatus::Cancelled;
                        env.storage()
                            .instance()
                            .set(&DataKey::Offer(offer_id), &offer);
                    }
                }
            }
        }
    }

    // ──────────────────────────────────────────────────────────
    // GETTER FUNCTIONS
    // ──────────────────────────────────────────────────────────

    /// Get listing details
    pub fn get_listing(env: Env, listing_id: u64) -> Option<Listing> {
        env.storage().instance().get(&DataKey::Listing(listing_id))
    }

    /// Get offer details
    pub fn get_offer(env: Env, offer_id: u64) -> Option<Offer> {
        env.storage().instance().get(&DataKey::Offer(offer_id))
    }

    /// Get counter offer details
    pub fn get_counter_offer(env: Env, counter_offer_id: u64) -> Option<CounterOffer> {
        env.storage().instance().get(&DataKey::CounterOffer(counter_offer_id))
    }

    /// Get all listings by seller
    pub fn get_listings_by_seller(env: &Env, seller: &Address) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::ListingsBySeller(seller.clone()))
            .unwrap_or(Vec::new(env))
    }

    /// Get all listings for an asset
    pub fn get_listings_by_asset(env: &Env, contract: &Address, token_id: &u32) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::ListingsByAsset(contract.clone(), *token_id))
            .unwrap_or(Vec::new(env))
    }

    /// Get all active listings
    pub fn get_active_listings(env: &Env) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::ActiveListings)
            .unwrap_or(Vec::new(env))
    }

    /// Get all offers for a listing
    pub fn get_offers_by_listing(env: &Env, listing_id: u64) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::OffersByListing(listing_id))
            .unwrap_or(Vec::new(env))
    }

    /// Get all counter offers for an offer
    pub fn get_counter_offers_by_offer(env: &Env, offer_id: u64) -> Vec<u64> {
        env.storage()
            .instance()
            .get(&DataKey::CounterOffersByOffer(offer_id))
            .unwrap_or(Vec::new(env))
    }

    /// Get price history for an asset
    pub fn get_price_history(env: Env, contract: Address, token_id: u32) -> Vec<i128> {
        env.storage()
            .instance()
            .get(&DataKey::PriceHistory(contract, token_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Get average price from history
    pub fn get_average_price(env: Env, contract: Address, token_id: u32) -> Option<i128> {
        let history = Self::get_price_history(env.clone(), contract, token_id);
        if history.is_empty() {
            return None;
        }

        let sum: i128 = history.iter().fold(0i128, |acc, &price| acc + price);
        Some(sum / history.len() as i128)
    }

    /// Get minimum price from history
    pub fn get_min_price(env: Env, contract: Address, token_id: u32) -> Option<i128> {
        let history = Self::get_price_history(env.clone(), contract, token_id);
        if history.is_empty() {
            return None;
        }

        let mut min = history.get(0).unwrap();
        for price in history.iter() {
            if price < min {
                min = price;
            }
        }
        Some(*min)
    }

    /// Get maximum price from history
    pub fn get_max_price(env: Env, contract: Address, token_id: u32) -> Option<i128> {
        let history = Self::get_price_history(env.clone(), contract, token_id);
        if history.is_empty() {
            return None;
        }

        let mut max = history.get(0).unwrap();
        for price in history.iter() {
            if price > max {
                max = price;
            }
        }
        Some(*max)
    }

    /// Get marketplace configuration
    pub fn get_config(env: Env) -> MarketplaceConfig {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .expect("Not initialized")
    }
}

#[cfg(test)]
mod test;

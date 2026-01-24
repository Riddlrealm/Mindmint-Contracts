#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

#[test]
fn test_bidding_and_refunds() {
    let env = Env::default();
    // 1. Mock Signatures (Essential for testing transfers)
    env.mock_all_auths();

    // 2. Setup a Fake Token (e.g., USDC)
    let token_admin = Address::generate(&env);
    // Note: Using the v2 registration to avoid deprecation warnings
    let token_contract_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();

    // These clients let us interact with the token
    let token_client = token::Client::new(&env, &token_contract_id);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_contract_id);

    // 3. Setup Auction Contract
    let contract_id = env.register_contract(None, AuctionContract);
    let client = AuctionContractClient::new(&env, &contract_id);
    client.init(&token_admin); // Init with random admin

    // 4. Setup Users & Balances
    let seller = Address::generate(&env);
    let bidder1 = Address::generate(&env);
    let bidder2 = Address::generate(&env);

    // Mint 1000 tokens to each bidder so they can play
    token_admin_client.mint(&bidder1, &1000);
    token_admin_client.mint(&bidder2, &1000);

    // 5. Create the Auction
    // Start time is 1000, End time is 2000
    let settings = AuctionSettings {
        start_time: 1000,
        end_time: 2000,
        starting_price: 100,
        reserve_price: 150,
        buy_now_price: 500,
        min_bid_increment: 10,
    };

    // create_auction args...
    let auction_id = client.create_auction(
        &seller,
        &Address::generate(&env), // Random NFT
        &1u64,
        &token_contract_id, // Use our fake token
        &AuctionType::English,
        &settings,
    );

    // Move time forward so the auction is "Open"
    env.ledger().set_timestamp(1500);

    // --- SCENARIO 1: First Bid ---
    // Bidder 1 bids 100 tokens
    client.place_bid(&bidder1, &auction_id, &100);

    // CHECK: Did the contract take the money?
    let auction = client.get_auction(&auction_id).unwrap();
    assert_eq!(auction.highest_bidder, Some(bidder1.clone()));
    assert_eq!(auction.current_bid, 100);

    // Bidder 1 should have 900 left (1000 - 100)
    assert_eq!(token_client.balance(&bidder1), 900);
    // Contract should hold 100
    assert_eq!(token_client.balance(&contract_id), 100);

    // --- SCENARIO 2: Outbid & Refund ---
    // Bidder 2 bids 120 tokens.
    // This should trigger a refund to Bidder 1.
    client.place_bid(&bidder2, &auction_id, &120);

    // CHECK: Did Bidder 1 get their refund?
    // Bidder 1 should be back to 1000
    assert_eq!(token_client.balance(&bidder1), 1000);

    // CHECK: Did Bidder 2 pay?
    // Bidder 2 should be down to 880 (1000 - 120)
    assert_eq!(token_client.balance(&bidder2), 880);

    // CHECK: Contract should now hold 120 (the new highest bid)
    assert_eq!(token_client.balance(&contract_id), 120);

    // CHECK: Who is winning?
    let auction_updated = client.get_auction(&auction_id).unwrap();
    assert_eq!(auction_updated.highest_bidder, Some(bidder2));
}

#[test]
fn test_dutch_auction_decay() {
    let env = Env::default();
    env.mock_all_auths();

    // 1. Setup Token & Users
    let token_admin = Address::generate(&env);
    let token_contract_id = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let token_client = token::Client::new(&env, &token_contract_id);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_contract_id);

    let contract_id = env.register_contract(None, AuctionContract);
    let client = AuctionContractClient::new(&env, &contract_id);
    client.init(&token_admin);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Give buyer 1000 tokens
    token_admin_client.mint(&buyer, &1000);

    // 2. Create Dutch Auction
    // Start: 500 | Reserve: 100 | Duration: 1000s (Time 0 to 1000)
    let settings = AuctionSettings {
        start_time: 0,
        end_time: 1000,
        starting_price: 500,
        reserve_price: 100,
        buy_now_price: 0, // Not used in Dutch logic usually, or same as start
        min_bid_increment: 0,
    };

    let auction_id = client.create_auction(
        &seller,
        &Address::generate(&env),
        &1u64,
        &token_contract_id,
        &AuctionType::Dutch,
        &settings,
    );

    // 3. Fast Forward Time to halfway (500 seconds)
    // Price drop range is 400 (500 - 100).
    // Halfway through, price should drop by 200.
    // Expected Price: 300.
    env.ledger().set_timestamp(500);

    // 4. Attempt to Buy
    // Buyer is willing to pay up to 305.
    // Since current price (300) < 305, this should succeed.
    client.buy_dutch(&buyer, &auction_id, &305);

    // 5. Verification
    let auction = client.get_auction(&auction_id).unwrap();

    // Status checks
    assert_eq!(auction.settled, true);
    assert_eq!(auction.highest_bidder, Some(buyer.clone()));

    // Price check: Did they pay 300?
    assert_eq!(auction.current_bid, 300);

    // Balance check: Buyer started with 1000, paid 300. Remainder: 700.
    assert_eq!(token_client.balance(&buyer), 700);
}
#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::Client as TokenClient,
    token::StellarAssetClient,
    Address, Env, Vec,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (Address, TokenClient<'a>, StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let addr = sac.address();
    (
        addr.clone(),
        TokenClient::new(env, &addr),
        StellarAssetClient::new(env, &addr),
    )
}

struct TestSetup<'a> {
    env: Env,
    client: TournamentPrizePoolContractClient<'a>,
    contract_addr: Address,
    admin: Address,
    oracle: Address,
    organiser: Address,
    token_addr: Address,
    token_client: TokenClient<'a>,
    token_admin_client: StellarAssetClient<'a>,
}

fn setup() -> TestSetup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let organiser = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_addr, token_client, token_admin_client) = create_token(&env, &token_admin);

    let contract_addr = env.register_contract(None, TournamentPrizePoolContract);
    let client = TournamentPrizePoolContractClient::new(&env, &contract_addr);

    client.initialize(&admin, &oracle, &token_addr);

    // Mint tokens to organiser
    token_admin_client.mint(&organiser, &100_000);

    TestSetup {
        env,
        client,
        contract_addr,
        admin,
        oracle,
        organiser,
        token_addr,
        token_client,
        token_admin_client,
    }
}

fn make_splits(env: &Env, bps: &[u32]) -> Vec<u32> {
    let mut v = Vec::new(env);
    for &b in bps {
        v.push_back(b);
    }
    v
}

fn make_players(env: &Env, count: usize) -> Vec<Address> {
    let mut v = Vec::new(env);
    for _ in 0..count {
        v.push_back(Address::generate(env));
    }
    v
}

// ──────────────────────────────────────────────────────────
// INITIALIZATION TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_initialize_success() {
    let s = setup();
    // Contract should be initialized — lock_fund should work
    let splits = make_splits(&s.env, &[10000]);
    let id = s.client.lock_fund(&s.organiser, &1000, &splits);
    assert_eq!(id, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_twice_fails() {
    let s = setup();
    s.client.initialize(&s.admin, &s.oracle, &s.token_addr);
}

// ──────────────────────────────────────────────────────────
// LOCK FUND TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_lock_fund_single_tier() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]); // 100% to winner
    let id = s.client.lock_fund(&s.organiser, &5000, &splits);
    assert_eq!(id, 0);

    let pool = s.client.get_pool(&id);
    assert_eq!(pool.total_fund, 5000);
    assert_eq!(pool.status, TournamentStatus::Locked);
    assert_eq!(pool.distributed, false);

    // Tokens moved from organiser to contract
    assert_eq!(s.token_client.balance(&s.organiser), 95_000);
    assert_eq!(s.token_client.balance(&s.contract_addr), 5000);
}

#[test]
fn test_lock_fund_three_tiers() {
    let s = setup();
    // 50%, 30%, 20%
    let splits = make_splits(&s.env, &[5000, 3000, 2000]);
    let id = s.client.lock_fund(&s.organiser, &10_000, &splits);
    assert_eq!(id, 0);

    let pool = s.client.get_pool(&id);
    assert_eq!(pool.tier_splits.len(), 3);
}

#[test]
fn test_lock_fund_increments_id() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]);

    let id1 = s.client.lock_fund(&s.organiser, &1000, &splits);
    let id2 = s.client.lock_fund(&s.organiser, &2000, &splits);
    let id3 = s.client.lock_fund(&s.organiser, &3000, &splits);

    assert_eq!(id1, 0);
    assert_eq!(id2, 1);
    assert_eq!(id3, 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_lock_fund_splits_not_100_percent() {
    let s = setup();
    let splits = make_splits(&s.env, &[5000, 3000]); // 80% != 100%
    s.client.lock_fund(&s.organiser, &1000, &splits);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_lock_fund_empty_splits() {
    let s = setup();
    let splits = make_splits(&s.env, &[]);
    s.client.lock_fund(&s.organiser, &1000, &splits);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_lock_fund_zero_amount() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]);
    s.client.lock_fund(&s.organiser, &0, &splits);
}

// ──────────────────────────────────────────────────────────
// SUBMIT STANDINGS TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_submit_standings_success() {
    let s = setup();
    let splits = make_splits(&s.env, &[5000, 3000, 2000]);
    let id = s.client.lock_fund(&s.organiser, &10_000, &splits);

    let players = make_players(&s.env, 3);
    s.client.submit_standings(&id, &players);

    let pool = s.client.get_pool(&id);
    assert_eq!(pool.status, TournamentStatus::StandingsSubmitted);

    let standings = s.client.get_standings(&id);
    assert_eq!(standings.len(), 3);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_submit_standings_wrong_count() {
    let s = setup();
    let splits = make_splits(&s.env, &[5000, 3000, 2000]); // 3 tiers
    let id = s.client.lock_fund(&s.organiser, &10_000, &splits);

    let players = make_players(&s.env, 2); // only 2 players
    s.client.submit_standings(&id, &players);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_submit_standings_on_cancelled() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]);
    let id = s.client.lock_fund(&s.organiser, &1000, &splits);

    s.client.cancel(&id);

    let players = make_players(&s.env, 1);
    s.client.submit_standings(&id, &players);
}

// ──────────────────────────────────────────────────────────
// DISTRIBUTION TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_distribute_single_winner() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]); // 100% to 1st
    let id = s.client.lock_fund(&s.organiser, &10_000, &splits);

    let winner = Address::generate(&s.env);
    let mut players = Vec::new(&s.env);
    players.push_back(winner.clone());
    s.client.submit_standings(&id, &players);

    s.client.distribute(&id);

    assert_eq!(s.token_client.balance(&winner), 10_000);
    assert_eq!(s.token_client.balance(&s.contract_addr), 0);

    let pool = s.client.get_pool(&id);
    assert_eq!(pool.status, TournamentStatus::Distributed);
    assert_eq!(pool.distributed, true);
}

#[test]
fn test_distribute_three_tiers() {
    let s = setup();
    // 50%, 30%, 20%
    let splits = make_splits(&s.env, &[5000, 3000, 2000]);
    let id = s.client.lock_fund(&s.organiser, &10_000, &splits);

    let p1 = Address::generate(&s.env);
    let p2 = Address::generate(&s.env);
    let p3 = Address::generate(&s.env);
    let mut players = Vec::new(&s.env);
    players.push_back(p1.clone());
    players.push_back(p2.clone());
    players.push_back(p3.clone());

    s.client.submit_standings(&id, &players);
    s.client.distribute(&id);

    assert_eq!(s.token_client.balance(&p1), 5000); // 50%
    assert_eq!(s.token_client.balance(&p2), 3000); // 30%
    assert_eq!(s.token_client.balance(&p3), 2000); // 20%
    assert_eq!(s.token_client.balance(&s.contract_addr), 0);
}

#[test]
fn test_distribute_five_tiers_precise() {
    let s = setup();
    // 40%, 25%, 15%, 12%, 8%
    let splits = make_splits(&s.env, &[4000, 2500, 1500, 1200, 800]);
    let id = s.client.lock_fund(&s.organiser, &20_000, &splits);

    let players = make_players(&s.env, 5);
    s.client.submit_standings(&id, &players);
    s.client.distribute(&id);

    assert_eq!(s.token_client.balance(&players.get(0).unwrap()), 8000);  // 40%
    assert_eq!(s.token_client.balance(&players.get(1).unwrap()), 5000);  // 25%
    assert_eq!(s.token_client.balance(&players.get(2).unwrap()), 3000);  // 15%
    assert_eq!(s.token_client.balance(&players.get(3).unwrap()), 2400);  // 12%
    assert_eq!(s.token_client.balance(&players.get(4).unwrap()), 1600);  // 8%
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_distribute_before_standings() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]);
    let id = s.client.lock_fund(&s.organiser, &1000, &splits);

    // Try to distribute without standings
    s.client.distribute(&id);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_distribute_twice_fails() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]);
    let id = s.client.lock_fund(&s.organiser, &1000, &splits);

    let players = make_players(&s.env, 1);
    s.client.submit_standings(&id, &players);
    s.client.distribute(&id);

    // Second distribute should fail
    s.client.distribute(&id);
}

// ──────────────────────────────────────────────────────────
// CANCEL / REFUND TESTS
// ──────────────────────────────────────────────────────────

#[test]
fn test_cancel_refunds_organiser() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]);
    let id = s.client.lock_fund(&s.organiser, &5000, &splits);

    assert_eq!(s.token_client.balance(&s.organiser), 95_000);

    s.client.cancel(&id);

    // Organiser gets full refund
    assert_eq!(s.token_client.balance(&s.organiser), 100_000);
    assert_eq!(s.token_client.balance(&s.contract_addr), 0);

    let pool = s.client.get_pool(&id);
    assert_eq!(pool.status, TournamentStatus::Cancelled);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_cancel_after_standings_fails() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]);
    let id = s.client.lock_fund(&s.organiser, &1000, &splits);

    let players = make_players(&s.env, 1);
    s.client.submit_standings(&id, &players);

    // Can't cancel after standings submitted
    s.client.cancel(&id);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_cancel_already_cancelled() {
    let s = setup();
    let splits = make_splits(&s.env, &[10000]);
    let id = s.client.lock_fund(&s.organiser, &1000, &splits);

    s.client.cancel(&id);
    // Double cancel should fail
    s.client.cancel(&id);
}

// ──────────────────────────────────────────────────────────
// VIEW FUNCTION TESTS
// ──────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_get_pool_nonexistent() {
    let s = setup();
    s.client.get_pool(&99);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_get_standings_nonexistent() {
    let s = setup();
    s.client.get_standings(&99);
}

// ──────────────────────────────────────────────────────────
// ADMIN FUNCTIONS
// ──────────────────────────────────────────────────────────

#[test]
fn test_set_oracle() {
    let s = setup();
    let new_oracle = Address::generate(&s.env);
    s.client.set_oracle(&new_oracle);

    // New oracle can submit standings
    let splits = make_splits(&s.env, &[10000]);
    let id = s.client.lock_fund(&s.organiser, &1000, &splits);
    let players = make_players(&s.env, 1);
    s.client.submit_standings(&id, &players);

    let pool = s.client.get_pool(&id);
    assert_eq!(pool.status, TournamentStatus::StandingsSubmitted);
}

// ──────────────────────────────────────────────────────────
// FULL LIFECYCLE TEST
// ──────────────────────────────────────────────────────────

#[test]
fn test_full_lifecycle() {
    let s = setup();

    // 1. Organiser locks fund with 3-tier split: 60%, 30%, 10%
    let splits = make_splits(&s.env, &[6000, 3000, 1000]);
    let id = s.client.lock_fund(&s.organiser, &50_000, &splits);

    assert_eq!(s.token_client.balance(&s.organiser), 50_000);
    assert_eq!(s.token_client.balance(&s.contract_addr), 50_000);

    // 2. Oracle submits standings
    let first = Address::generate(&s.env);
    let second = Address::generate(&s.env);
    let third = Address::generate(&s.env);
    let mut ranked = Vec::new(&s.env);
    ranked.push_back(first.clone());
    ranked.push_back(second.clone());
    ranked.push_back(third.clone());

    s.client.submit_standings(&id, &ranked);

    // 3. Anyone distributes
    s.client.distribute(&id);

    // 4. Verify payouts
    assert_eq!(s.token_client.balance(&first), 30_000);   // 60%
    assert_eq!(s.token_client.balance(&second), 15_000);   // 30%
    assert_eq!(s.token_client.balance(&third), 5_000);     // 10%
    assert_eq!(s.token_client.balance(&s.contract_addr), 0);

    // 5. Pool is marked as distributed
    let pool = s.client.get_pool(&id);
    assert_eq!(pool.status, TournamentStatus::Distributed);
    assert_eq!(pool.distributed, true);
}

#[test]
fn test_multiple_tournaments() {
    let s = setup();

    // Tournament A: winner takes all
    let splits_a = make_splits(&s.env, &[10000]);
    let id_a = s.client.lock_fund(&s.organiser, &10_000, &splits_a);

    // Tournament B: 70/30 split
    let splits_b = make_splits(&s.env, &[7000, 3000]);
    let id_b = s.client.lock_fund(&s.organiser, &20_000, &splits_b);

    // Submit and distribute A
    let winner_a = Address::generate(&s.env);
    let mut ranked_a = Vec::new(&s.env);
    ranked_a.push_back(winner_a.clone());
    s.client.submit_standings(&id_a, &ranked_a);
    s.client.distribute(&id_a);

    assert_eq!(s.token_client.balance(&winner_a), 10_000);

    // Cancel B
    s.client.cancel(&id_b);
    assert_eq!(s.token_client.balance(&s.organiser), 90_000); // 100k - 10k(A) - 20k(B) + 20k(refund)

    // Verify independent statuses
    assert_eq!(s.client.get_pool(&id_a).status, TournamentStatus::Distributed);
    assert_eq!(s.client.get_pool(&id_b).status, TournamentStatus::Cancelled);
}

#![cfg(test)]

use crate::{
    Condition, ConditionType, ConditionalRewardContract, ConditionalRewardContractClient,
    PlayerIdentityView, SocialLinksView, SubscriptionTier,
};
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, IntoVal, Map, String, Symbol, Val, Vec,
};

#[contract]
struct MockNft;

#[contractimpl]
impl MockNft {
    pub fn set_owner(env: Env, token_id: u32, owner: Address) {
        env.storage()
            .persistent()
            .set(&(symbol_short!("OWNER"), token_id), &owner);
    }

    pub fn owner_of(env: Env, token_id: u32) -> Address {
        env.storage()
            .persistent()
            .get(&(symbol_short!("OWNER"), token_id))
            .unwrap()
    }
}

#[contract]
struct MockProof;

#[contractimpl]
impl MockProof {
    pub fn set_count(env: Env, player: Address, activity_type: u32, count: u32) {
        env.storage()
            .persistent()
            .set(&(symbol_short!("CNT"), player, activity_type), &count);
    }

    pub fn get_activity_count(env: Env, player: Address, activity_type: u32) -> u32 {
        env.storage()
            .persistent()
            .get(&(symbol_short!("CNT"), player, activity_type))
            .unwrap_or(0)
    }
}

#[contract]
struct MockIdentity;

#[contractimpl]
impl MockIdentity {
    pub fn set_identity(env: Env, player: Address, registered_at: u64) {
        let identity = PlayerIdentityView {
            address: player.clone(),
            username: String::from_str(&env, "player"),
            avatar_hash: None,
            bio_hash: None,
            social_links: SocialLinksView {
                twitter: None,
                discord: None,
                github: None,
            },
            registered_at,
            verified: true,
        };

        env.storage()
            .persistent()
            .set(&(symbol_short!("ID"), player), &identity);
    }

    pub fn resolve_address(env: Env, player: Address) -> Option<PlayerIdentityView> {
        env.storage()
            .persistent()
            .get(&(symbol_short!("ID"), player))
    }
}

#[contract]
struct MockSubscription;

#[contractimpl]
impl MockSubscription {
    pub fn set_tier(env: Env, player: Address, tier: SubscriptionTier) {
        env.storage()
            .persistent()
            .set(&(symbol_short!("TIER"), player), &tier);
    }

    pub fn has_access(env: Env, player: Address, required_tier: SubscriptionTier) -> bool {
        let tier: SubscriptionTier = env
            .storage()
            .persistent()
            .get(&(symbol_short!("TIER"), player))
            .unwrap_or(SubscriptionTier::Free);
        (tier as u32) >= (required_tier as u32)
    }
}

struct TestContext {
    env: Env,
    admin: Address,
    player: Address,
    other_player: Address,
    contract: ConditionalRewardContractClient<'static>,
    token: TokenClient<'static>,
    nft: Address,
    proof: Address,
    identity: Address,
    subscription: Address,
}

impl TestContext {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(100);

        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        let other_player = Address::generate(&env);
        let token_admin = Address::generate(&env);

        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let token = TokenClient::new(&env, &token_id);
        let asset_admin = StellarAssetClient::new(&env, &token_id);

        let contract_id = env.register_contract(None, ConditionalRewardContract);
        let contract = ConditionalRewardContractClient::new(&env, &contract_id);
        contract.initialize(&admin, &token_id);

        let nft = env.register_contract(None, MockNft);
        let proof = env.register_contract(None, MockProof);
        let identity = env.register_contract(None, MockIdentity);
        let subscription = env.register_contract(None, MockSubscription);

        asset_admin.mint(&admin, &10_000);

        Self {
            env,
            admin,
            player,
            other_player,
            contract,
            token,
            nft,
            proof,
            identity,
            subscription,
        }
    }
}

fn params(env: &Env, entries: &[(Symbol, Val)]) -> Map<Symbol, Val> {
    let mut map = Map::new(env);
    for (key, value) in entries.iter() {
        map.set(key.clone(), value.clone());
    }
    map
}

fn nft_condition(env: &Env, contract: &Address, token_id: u32, or_group: u32) -> Condition {
    Condition {
        condition_type: ConditionType::NftHeld,
        params: params(
            env,
            &[
                (symbol_short!("contract"), contract.clone().into_val(env)),
                (symbol_short!("token_id"), token_id.into_val(env)),
            ],
        ),
        or_group,
    }
}

fn solve_condition(
    env: &Env,
    contract: &Address,
    min: u32,
    activity_type: u32,
    or_group: u32,
) -> Condition {
    Condition {
        condition_type: ConditionType::SolveCountGte,
        params: params(
            env,
            &[
                (symbol_short!("contract"), contract.clone().into_val(env)),
                (symbol_short!("min"), min.into_val(env)),
                (symbol_short!("activity"), activity_type.into_val(env)),
            ],
        ),
        or_group,
    }
}

fn registration_condition(env: &Env, contract: &Address, days: u64, or_group: u32) -> Condition {
    Condition {
        condition_type: ConditionType::RegistrationAgeGte,
        params: params(
            env,
            &[
                (symbol_short!("contract"), contract.clone().into_val(env)),
                (symbol_short!("days"), days.into_val(env)),
            ],
        ),
        or_group,
    }
}

fn tier_condition(
    env: &Env,
    contract: &Address,
    tier: SubscriptionTier,
    or_group: u32,
) -> Condition {
    Condition {
        condition_type: ConditionType::TierGte,
        params: params(
            env,
            &[
                (symbol_short!("contract"), contract.clone().into_val(env)),
                (symbol_short!("tier"), (tier as u32).into_val(env)),
            ],
        ),
        or_group,
    }
}

fn configure_player(ctx: &TestContext) {
    MockNftClient::new(&ctx.env, &ctx.nft).set_owner(&7u32, &ctx.player);
    MockProofClient::new(&ctx.env, &ctx.proof).set_count(&ctx.player, &0u32, &60u32);
    MockIdentityClient::new(&ctx.env, &ctx.identity).set_identity(&ctx.player, &60u64);
    MockSubscriptionClient::new(&ctx.env, &ctx.subscription)
        .set_tier(&ctx.player, &SubscriptionTier::Elite);
}

#[test]
fn test_eligible_claim() {
    let ctx = TestContext::new();
    configure_player(&ctx);

    let conditions = Vec::from_array(
        &ctx.env,
        [
            nft_condition(&ctx.env, &ctx.nft, 7, 0),
            solve_condition(&ctx.env, &ctx.proof, 50, 0, 0),
            registration_condition(&ctx.env, &ctx.identity, 30, 0),
            tier_condition(&ctx.env, &ctx.subscription, SubscriptionTier::Pro, 1),
            tier_condition(&ctx.env, &ctx.subscription, SubscriptionTier::Elite, 1),
        ],
    );

    let reward_id = ctx
        .contract
        .create_reward(&ctx.admin, &conditions, &100i128, &2u32);
    let (eligible, failed) = ctx.contract.check_eligibility(&reward_id, &ctx.player);
    assert!(eligible);
    assert!(failed.is_empty());

    let admin_before = ctx.token.balance(&ctx.admin);
    let player_before = ctx.token.balance(&ctx.player);
    ctx.contract.claim(&reward_id, &ctx.player);

    assert_eq!(ctx.token.balance(&ctx.player), player_before + 100);
    assert_eq!(ctx.token.balance(&ctx.admin), admin_before);

    let reward = ctx.contract.get_reward(&reward_id);
    assert!(reward.active);
    assert_eq!(reward.claims_remaining, 1);
}

#[test]
fn test_partially_failing_conditions_reported() {
    let ctx = TestContext::new();

    MockNftClient::new(&ctx.env, &ctx.nft).set_owner(&7u32, &ctx.player);
    MockProofClient::new(&ctx.env, &ctx.proof).set_count(&ctx.player, &0u32, &12u32);
    MockIdentityClient::new(&ctx.env, &ctx.identity).set_identity(&ctx.player, &80u64);
    MockSubscriptionClient::new(&ctx.env, &ctx.subscription)
        .set_tier(&ctx.player, &SubscriptionTier::Free);

    let conditions = Vec::from_array(
        &ctx.env,
        [
            nft_condition(&ctx.env, &ctx.nft, 7, 0),
            solve_condition(&ctx.env, &ctx.proof, 50, 0, 0),
            registration_condition(&ctx.env, &ctx.identity, 10, 0),
            tier_condition(&ctx.env, &ctx.subscription, SubscriptionTier::Pro, 2),
            tier_condition(&ctx.env, &ctx.subscription, SubscriptionTier::Elite, 2),
        ],
    );

    let reward_id = ctx
        .contract
        .create_reward(&ctx.admin, &conditions, &100i128, &1u32);
    let (eligible, failed) = ctx.contract.check_eligibility(&reward_id, &ctx.player);

    assert!(!eligible);
    assert_eq!(failed.len(), 3);
    assert_eq!(failed.get(0).unwrap().index, 1);
    assert_eq!(failed.get(1).unwrap().or_group, 2);
    assert_eq!(failed.get(2).unwrap().or_group, 2);
}

#[test]
#[should_panic(expected = "max claims reached")]
fn test_max_claims_cap() {
    let ctx = TestContext::new();
    configure_player(&ctx);
    MockProofClient::new(&ctx.env, &ctx.proof).set_count(&ctx.other_player, &0u32, &60u32);
    MockIdentityClient::new(&ctx.env, &ctx.identity).set_identity(&ctx.other_player, &60u64);
    MockSubscriptionClient::new(&ctx.env, &ctx.subscription)
        .set_tier(&ctx.other_player, &SubscriptionTier::Elite);

    let conditions = Vec::from_array(
        &ctx.env,
        [
            solve_condition(&ctx.env, &ctx.proof, 50, 0, 0),
            registration_condition(&ctx.env, &ctx.identity, 30, 0),
            tier_condition(&ctx.env, &ctx.subscription, SubscriptionTier::Elite, 0),
        ],
    );

    let reward_id = ctx
        .contract
        .create_reward(&ctx.admin, &conditions, &100i128, &1u32);
    ctx.contract.claim(&reward_id, &ctx.player);
    ctx.contract.claim(&reward_id, &ctx.other_player);
}

#[test]
fn test_deactivation_refunds_unclaimed_funds() {
    let ctx = TestContext::new();
    configure_player(&ctx);

    let conditions = Vec::from_array(
        &ctx.env,
        [
            nft_condition(&ctx.env, &ctx.nft, 7, 0),
            solve_condition(&ctx.env, &ctx.proof, 50, 0, 0),
            registration_condition(&ctx.env, &ctx.identity, 30, 0),
            tier_condition(&ctx.env, &ctx.subscription, SubscriptionTier::Pro, 0),
        ],
    );

    let reward_id = ctx
        .contract
        .create_reward(&ctx.admin, &conditions, &100i128, &3u32);
    ctx.contract.claim(&reward_id, &ctx.player);

    let admin_before = ctx.token.balance(&ctx.admin);
    let refund = ctx.contract.deactivate_reward(&ctx.admin, &reward_id);
    assert_eq!(refund, 200);
    assert_eq!(ctx.token.balance(&ctx.admin), admin_before + 200);

    let reward = ctx.contract.get_reward(&reward_id);
    assert!(!reward.active);
    assert_eq!(reward.claims_remaining, 2);
}

#[test]
#[should_panic(expected = "player already claimed")]
fn test_duplicate_claim_rejected() {
    let ctx = TestContext::new();
    configure_player(&ctx);

    let conditions = Vec::from_array(
        &ctx.env,
        [
            nft_condition(&ctx.env, &ctx.nft, 7, 0),
            solve_condition(&ctx.env, &ctx.proof, 50, 0, 0),
            registration_condition(&ctx.env, &ctx.identity, 30, 0),
            tier_condition(&ctx.env, &ctx.subscription, SubscriptionTier::Pro, 0),
        ],
    );

    let reward_id = ctx
        .contract
        .create_reward(&ctx.admin, &conditions, &100i128, &2u32);
    ctx.contract.claim(&reward_id, &ctx.player);
    ctx.contract.claim(&reward_id, &ctx.player);
}

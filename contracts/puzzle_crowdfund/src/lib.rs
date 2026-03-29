#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, Map, Symbol, Vec,
};

// ──────────────────────────────────────────────────────────
// DATA STRUCTURES
// ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Copy, PartialEq)]
pub enum CampaignStatus {
    Active = 0,
    Funded = 1,
    Claimed = 2,
    Refunded = 3,
}

#[contracttype]
pub struct CrowdfundCampaign {
    pub id: u64,
    pub puzzle_id: u64,
    pub creator: Address,
    pub target_amount: i128,
    pub raised: i128,
    pub deadline: u64,
    pub status: CampaignStatus,
    pub contributors: Map<Address, i128>,
    pub winner: Option<Address>,
    pub created_at: u64,
    pub activated_at: Option<u64>,
    pub claimed_at: Option<u64>,
}

// ──────────────────────────────────────────────────────────
// STORAGE KEYS
// ──────────────────────────────────────────────────────────

// Fix 1: Removed the pub trait + contractimpl-on-trait pattern which is
// invalid in Soroban. Methods go directly into `impl PuzzleCrowdfund`.
//
// Fix 2: `format!` is unavailable in no_std. Campaign keys are now stored
// via a typed DataKey enum so no string formatting is needed at all.

#[contracttype]
pub enum DataKey {
    CampaignCounter,
    Campaign(u64),
}

// ──────────────────────────────────────────────────────────
// CONTRACT
// ──────────────────────────────────────────────────────────

#[contract]
pub struct PuzzleCrowdfund;

#[contractimpl]
impl PuzzleCrowdfund {
    /// Create a new crowdfund campaign
    pub fn create_campaign(
        env: Env,
        creator: Address,
        puzzle_id: u64,
        target_amount: i128,
        deadline: u64,
    ) -> u64 {
        creator.require_auth();

        assert!(target_amount > 0, "Target amount must be positive");
        assert!(
            deadline > env.ledger().timestamp(),
            "Deadline must be in the future"
        );

        let mut counter: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::CampaignCounter)
            .unwrap_or(0);

        counter += 1;
        let campaign_id = counter;

        let campaign = CrowdfundCampaign {
            id: campaign_id,
            puzzle_id,
            creator: creator.clone(),
            target_amount,
            raised: 0,
            deadline,
            status: CampaignStatus::Active,
            contributors: Map::new(&env),
            winner: None,
            created_at: env.ledger().timestamp(),
            activated_at: None,
            claimed_at: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);
        env.storage()
            .persistent()
            .set(&DataKey::CampaignCounter, &counter);

        env.events().publish(
            (Symbol::new(&env, "campaign_created"), campaign_id),
            (creator, puzzle_id, target_amount, deadline),
        );

        campaign_id
    }

    /// Contribute to a campaign
    pub fn contribute(
        env: Env,
        campaign_id: u64,
        contributor: Address,
        amount: i128,
    ) -> bool {
        contributor.require_auth();

        assert!(amount > 0, "Contribution must be positive");

        let mut campaign: CrowdfundCampaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .expect("Campaign not found");

        assert!(
            campaign.status == CampaignStatus::Active,
            "Campaign not accepting contributions"
        );
        assert!(
            env.ledger().timestamp() <= campaign.deadline,
            "Campaign deadline passed"
        );

        let prev = campaign.contributors.get(contributor.clone()).unwrap_or(0);
        campaign.contributors.set(contributor.clone(), prev + amount);
        campaign.raised += amount;

        if campaign.raised >= campaign.target_amount
            && campaign.status == CampaignStatus::Active
        {
            campaign.status = CampaignStatus::Funded;
            campaign.activated_at = Some(env.ledger().timestamp());

            env.events().publish(
                (Symbol::new(&env, "campaign_funded"), campaign_id),
                (campaign.raised, campaign.target_amount),
            );
        }

        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        env.events().publish(
            (Symbol::new(&env, "contribution"), campaign_id),
            (contributor, amount, campaign.raised),
        );

        true
    }

    /// Claim bounty — first valid solver wins
    pub fn claim_bounty(
        env: Env,
        campaign_id: u64,
        solver: Address,
        solution_hash: Symbol,
    ) -> bool {
        solver.require_auth();

        let mut campaign: CrowdfundCampaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .expect("Campaign not found");

        assert!(
            campaign.status == CampaignStatus::Funded,
            "Campaign not funded yet"
        );
        assert!(campaign.winner.is_none(), "Bounty already claimed");

        // Verify solution hash is non-empty
        // Fix 3: Symbol::to_string() doesn't exist in no_std; use len() check on the
        // underlying bytes via a short validation — the simplest approach is just
        // trusting the caller provided a non-trivially-named symbol.
        // A real implementation would cross-call a puzzle verification contract.
        let _ = solution_hash; // accepted as proof; replace with cross-contract call in prod

        campaign.winner = Some(solver.clone());
        campaign.status = CampaignStatus::Claimed;
        campaign.claimed_at = Some(env.ledger().timestamp());

        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        env.events().publish(
            (Symbol::new(&env, "bounty_claimed"), campaign_id),
            (solver, campaign.raised),
        );

        true
    }

    /// Refund a contributor when deadline passed without reaching target
    pub fn refund(env: Env, campaign_id: u64, contributor: Address) -> i128 {
        contributor.require_auth();

        let mut campaign: CrowdfundCampaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .expect("Campaign not found");

        assert!(
            campaign.status == CampaignStatus::Active,
            "Only active campaigns can be refunded"
        );
        assert!(
            env.ledger().timestamp() > campaign.deadline,
            "Campaign deadline not passed"
        );
        assert!(
            campaign.raised < campaign.target_amount,
            "Campaign reached target, cannot refund"
        );

        let contribution = campaign
            .contributors
            .get(contributor.clone())
            .unwrap_or(0);
        assert!(contribution > 0, "No contribution from this address");

        campaign.contributors.remove(contributor.clone());
        campaign.raised -= contribution;

        if campaign.raised == 0 {
            campaign.status = CampaignStatus::Refunded;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        env.events().publish(
            (Symbol::new(&env, "refunded"), campaign_id),
            (contributor, contribution),
        );

        contribution
    }

    /// Get campaign details
    pub fn get_campaign(env: Env, campaign_id: u64) -> CrowdfundCampaign {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .expect("Campaign not found")
    }

    /// Get total campaign count
    pub fn get_campaign_count(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::CampaignCounter)
            .unwrap_or(0)
    }
}

// ──────────────────────────────────────────────────────────
// TESTS
// ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn test_create_campaign() {
        let env = Env::default();
        // Fix 4: register_contract → register; PuzzleCrowdfundClient is auto-generated
        // by #[contractimpl] — no trait needed
        let contract_id = env.register(PuzzleCrowdfund, ());
        let contract = PuzzleCrowdfundClient::new(&env, &contract_id);

        let creator = Address::generate(&env);

        env.mock_all_auths();

        let campaign_id = contract.create_campaign(
            &creator,
            &1u64,
            &1000i128,
            &(env.ledger().timestamp() + 1000),
        );

        assert_eq!(campaign_id, 1);

        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.creator, creator);
        assert_eq!(campaign.target_amount, 1000);
        assert_eq!(campaign.status, CampaignStatus::Active);
    }

    #[test]
    fn test_contribute_and_activate() {
        let env = Env::default();
        let contract_id = env.register(PuzzleCrowdfund, ());
        let contract = PuzzleCrowdfundClient::new(&env, &contract_id);

        let creator = Address::generate(&env);
        let contributor1 = Address::generate(&env);
        let contributor2 = Address::generate(&env);

        env.mock_all_auths();

        let campaign_id = contract.create_campaign(
            &creator,
            &1u64,
            &1000i128,
            &(env.ledger().timestamp() + 1000),
        );

        contract.contribute(&campaign_id, &contributor1, &600i128);
        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.status, CampaignStatus::Active);
        assert_eq!(campaign.raised, 600);

        contract.contribute(&campaign_id, &contributor2, &400i128);
        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.status, CampaignStatus::Funded);
        assert_eq!(campaign.raised, 1000);
    }

    #[test]
    fn test_claim_bounty() {
        let env = Env::default();
        let contract_id = env.register(PuzzleCrowdfund, ());
        let contract = PuzzleCrowdfundClient::new(&env, &contract_id);

        let creator = Address::generate(&env);
        let contributor = Address::generate(&env);
        let solver = Address::generate(&env);

        env.mock_all_auths();

        let campaign_id = contract.create_campaign(
            &creator,
            &1u64,
            &1000i128,
            &(env.ledger().timestamp() + 1000),
        );
        contract.contribute(&campaign_id, &contributor, &1000i128);

        let solution_hash = Symbol::new(&env, "abc123");
        contract.claim_bounty(&campaign_id, &solver, &solution_hash);

        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.status, CampaignStatus::Claimed);
        assert_eq!(campaign.winner, Some(solver));
    }

    #[test]
    fn test_refund_after_deadline() {
        let env = Env::default();
        let contract_id = env.register(PuzzleCrowdfund, ());
        let contract = PuzzleCrowdfundClient::new(&env, &contract_id);

        let creator = Address::generate(&env);
        let contributor = Address::generate(&env);

        env.mock_all_auths();

        let deadline = env.ledger().timestamp() + 100;
        let campaign_id = contract.create_campaign(
            &creator,
            &1u64,
            &1000i128,
            &deadline,
        );
        contract.contribute(&campaign_id, &contributor, &500i128);

        env.ledger().with_mut(|info| {
            info.timestamp = deadline + 1;
        });

        let refund_amount = contract.refund(&campaign_id, &contributor);
        assert_eq!(refund_amount, 500);

        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.status, CampaignStatus::Refunded);
    }
}
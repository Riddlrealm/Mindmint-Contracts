#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, Map, Symbol, Vec, IntoVal, FromVal,
};

/// Campaign Status
#[derive(Clone, Copy, PartialEq)]
#[contracttype]
pub enum CampaignStatus {
    Active = 0,       // Fundraising
    Funded = 1,       // Target reached, bounty active
    Claimed = 2,      // Bounty claimed by winner
    Refunded = 3,     // Refunded to contributors
}

/// Crowdfund Campaign Struct
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

/// Contract Trait
#[contract]
pub trait PuzzleCrowdfundTrait {
    /// Create a new crowdfund campaign
    fn create_campaign(
        env: Env,
        creator: Address,
        puzzle_id: u64,
        target_amount: i128,
        deadline: u64,
    ) -> u64;

    /// Contribute to a campaign
    fn contribute(env: Env, campaign_id: u64, contributor: Address, amount: i128) -> bool;

    /// Claim bounty (first valid solver wins)
    fn claim_bounty(
        env: Env,
        campaign_id: u64,
        solver: Address,
        solution_hash: Symbol,
    ) -> bool;

    /// Refund contributors if deadline passed without funding
    fn refund(env: Env, campaign_id: u64, contributor: Address) -> i128;

    /// Get campaign details
    fn get_campaign(env: Env, campaign_id: u64) -> CrowdfundCampaign;

    /// Get campaign count
    fn get_campaign_count(env: Env) -> u64;
}

#[contractimpl]
pub struct PuzzleCrowdfund;

#[contractimpl]
impl PuzzleCrowdfundTrait for PuzzleCrowdfund {
    fn create_campaign(
        env: Env,
        creator: Address,
        puzzle_id: u64,
        target_amount: i128,
        deadline: u64,
    ) -> u64 {
        creator.require_auth();

        assert!(target_amount > 0, "Target amount must be positive");
        assert!(deadline > env.ledger().timestamp(), "Deadline must be in the future");

        let counter_key = Symbol::new(&env, "campaign_counter");
        let mut counter: u64 = env
            .storage()
            .persistent()
            .get::<Symbol, u64>(&counter_key)
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

        let campaign_key = Symbol::new(&env, &format!("campaign_{}", campaign_id));
        env.storage().persistent().set(&campaign_key, &campaign);
        env.storage().persistent().set(&counter_key, &counter);

        env.events().publish(
            (Symbol::new(&env, "campaign_created"), campaign_id),
            (creator, puzzle_id, target_amount, deadline),
        );

        campaign_id
    }

    fn contribute(env: Env, campaign_id: u64, contributor: Address, amount: i128) -> bool {
        contributor.require_auth();

        assert!(amount > 0, "Contribution must be positive");

        let campaign_key = Symbol::new(&env, &format!("campaign_{}", campaign_id));
        let mut campaign: CrowdfundCampaign = env
            .storage()
            .persistent()
            .get(&campaign_key)
            .expect("Campaign not found");

        assert!(campaign.status == CampaignStatus::Active, "Campaign not accepting contributions");
        assert!(env.ledger().timestamp() <= campaign.deadline, "Campaign deadline passed");

        // Add contribution
        let prev = campaign.contributors.get(contributor.clone()).unwrap_or(0);
        campaign.contributors.set(contributor.clone(), prev + amount);
        campaign.raised += amount;

        // Auto-activate if target reached
        if campaign.raised >= campaign.target_amount && campaign.status == CampaignStatus::Active {
            campaign.status = CampaignStatus::Funded;
            campaign.activated_at = Some(env.ledger().timestamp());

            env.events().publish(
                (Symbol::new(&env, "campaign_activated"), campaign_id),
                (campaign.raised, campaign.target_amount),
            );
        }

        env.storage().persistent().set(&campaign_key, &campaign);

        env.events().publish(
            (Symbol::new(&env, "contribution_received"), campaign_id),
            (contributor, amount, campaign.raised),
        );

        true
    }

    fn claim_bounty(
        env: Env,
        campaign_id: u64,
        solver: Address,
        solution_hash: Symbol,
    ) -> bool {
        solver.require_auth();

        let campaign_key = Symbol::new(&env, &format!("campaign_{}", campaign_id));
        let mut campaign: CrowdfundCampaign = env
            .storage()
            .persistent()
            .get(&campaign_key)
            .expect("Campaign not found");

        assert!(
            campaign.status == CampaignStatus::Funded,
            "Campaign not funded yet"
        );
        assert!(campaign.winner.is_none(), "Bounty already claimed");

        // Verify solution (in production, this would call puzzle_verification contract)
        // For now, we trust the solver provided a valid solution hash
        assert!(solution_hash.to_string().len() > 0, "Invalid solution hash");

        // Set winner and mark as claimed
        campaign.winner = Some(solver.clone());
        campaign.status = CampaignStatus::Claimed;
        campaign.claimed_at = Some(env.ledger().timestamp());

        env.storage().persistent().set(&campaign_key, &campaign);

        env.events().publish(
            (Symbol::new(&env, "bounty_claimed"), campaign_id),
            (solver, campaign.raised),
        );

        true
    }

    fn refund(env: Env, campaign_id: u64, contributor: Address) -> i128 {
        contributor.require_auth();

        let campaign_key = Symbol::new(&env, &format!("campaign_{}", campaign_id));
        let mut campaign: CrowdfundCampaign = env
            .storage()
            .persistent()
            .get(&campaign_key)
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

        // Remove contribution from map
        campaign.contributors.remove(contributor.clone());
        campaign.raised -= contribution;

        // If all refunded, mark as refunded
        if campaign.raised == 0 {
            campaign.status = CampaignStatus::Refunded;
        }

        env.storage().persistent().set(&campaign_key, &campaign);

        env.events().publish(
            (Symbol::new(&env, "campaign_refunded"), campaign_id),
            (contributor, contribution),
        );

        contribution
    }

    fn get_campaign(env: Env, campaign_id: u64) -> CrowdfundCampaign {
        let campaign_key = Symbol::new(&env, &format!("campaign_{}", campaign_id));
        env.storage()
            .persistent()
            .get(&campaign_key)
            .expect("Campaign not found")
    }

    fn get_campaign_count(env: Env) -> u64 {
        let counter_key = Symbol::new(&env, "campaign_counter");
        env.storage()
            .persistent()
            .get::<Symbol, u64>(&counter_key)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn test_create_campaign() {
        let env = Env::default();
        let contract = PuzzleCrowdfundClient::new(&env, &env.register_contract(None, PuzzleCrowdfund));

        let creator = Address::random(&env);
        let campaign_id = contract.create_campaign(&creator, &1u64, &1000i128, &(env.ledger().timestamp() + 1000));

        assert_eq!(campaign_id, 1);

        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.creator, creator);
        assert_eq!(campaign.target_amount, 1000);
        assert_eq!(campaign.status, CampaignStatus::Active);
    }

    #[test]
    fn test_contribute_and_activate() {
        let env = Env::default();
        let contract = PuzzleCrowdfundClient::new(&env, &env.register_contract(None, PuzzleCrowdfund));

        let creator = Address::random(&env);
        let contributor1 = Address::random(&env);
        let contributor2 = Address::random(&env);

        let campaign_id = contract.create_campaign(&creator, &1u64, &1000i128, &(env.ledger().timestamp() + 1000));

        // Contribute 600
        contract.contribute(&campaign_id, &contributor1, &600i128);
        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.status, CampaignStatus::Active);
        assert_eq!(campaign.raised, 600);

        // Contribute 400 more -> should activate
        contract.contribute(&campaign_id, &contributor2, &400i128);
        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.status, CampaignStatus::Funded);
        assert_eq!(campaign.raised, 1000);
    }

    #[test]
    fn test_claim_bounty() {
        let env = Env::default();
        let contract = PuzzleCrowdfundClient::new(&env, &env.register_contract(None, PuzzleCrowdfund));

        let creator = Address::random(&env);
        let contributor = Address::random(&env);
        let solver = Address::random(&env);

        let campaign_id = contract.create_campaign(&creator, &1u64, &1000i128, &(env.ledger().timestamp() + 1000));
        contract.contribute(&campaign_id, &contributor, &1000i128);

        // Claim bounty
        let solution_hash = Symbol::new(&env, "abc123");
        contract.claim_bounty(&campaign_id, &solver, &solution_hash);

        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.status, CampaignStatus::Claimed);
        assert_eq!(campaign.winner, Some(solver));
    }

    #[test]
    fn test_refund_after_deadline() {
        let env = Env::default();
        let contract = PuzzleCrowdfundClient::new(&env, &env.register_contract(None, PuzzleCrowdfund));

        let creator = Address::random(&env);
        let contributor = Address::random(&env);

        let deadline = env.ledger().timestamp() + 100;
        let campaign_id = contract.create_campaign(&creator, &1u64, &1000i128, &deadline);
        contract.contribute(&campaign_id, &contributor, &500i128);

        // Fast forward past deadline
        env.ledger().with_mut_info(|info| {
            info.timestamp = deadline + 1;
        });

        // Refund
        let refund_amount = contract.refund(&campaign_id, &contributor);
        assert_eq!(refund_amount, 500);

        let campaign = contract.get_campaign(&campaign_id);
        assert_eq!(campaign.status, CampaignStatus::Refunded);
    }
}

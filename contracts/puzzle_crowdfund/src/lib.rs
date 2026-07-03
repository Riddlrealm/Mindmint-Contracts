#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Map, String, Symbol, Vec,
};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CampaignStatus {
    Active,
    Successful,
    Failed,
    Completed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneInput {
    pub name: String,
    pub payout_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StretchGoalInput {
    pub target_amount: i128,
    pub description: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    pub id: u32,
    pub name: String,
    pub payout_amount: i128,
    pub approved: bool,
    pub claimed: bool,
    pub claimed_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StretchGoal {
    pub id: u32,
    pub target_amount: i128,
    pub description: String,
    pub reached: bool,
    pub reached_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContributionRecord {
    pub backer: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Campaign {
    pub id: u64,
    pub creator: Address,
    pub title: String,
    pub description: String,
    pub goal_amount: i128,
    pub deadline: u64,
    pub amount_raised: i128,
    pub amount_claimed: i128,
    pub refunded_amount: i128,
    pub status: CampaignStatus,
    pub created_at: u64,
    pub funded_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub contributions: Map<Address, i128>,
    pub refunded_backers: Map<Address, bool>,
    pub backers: Vec<Address>,
    pub contribution_history: Vec<ContributionRecord>,
    pub milestones: Vec<Milestone>,
    pub stretch_goals: Vec<StretchGoal>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignSummary {
    pub id: u64,
    pub creator: Address,
    pub goal_amount: i128,
    pub deadline: u64,
    pub amount_raised: i128,
    pub amount_claimed: i128,
    pub refunded_amount: i128,
    pub available_balance: i128,
    pub status: CampaignStatus,
    pub backer_count: u32,
    pub contribution_count: u32,
    pub goal_reached: bool,
    pub next_stretch_goal: Option<StretchGoal>,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Token,
    NextCampaignId,
    Campaign(u64),
}

#[contract]
pub struct PuzzleCrowdfund;

#[contractimpl]
impl PuzzleCrowdfund {
    pub fn initialize(env: Env, admin: Address, token: Address) {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::NextCampaignId, &1u64);
    }

    pub fn create_campaign(
        env: Env,
        creator: Address,
        title: String,
        description: String,
        goal_amount: i128,
        deadline: u64,
        milestone_inputs: Vec<MilestoneInput>,
        stretch_goal_inputs: Vec<StretchGoalInput>,
    ) -> u64 {
        creator.require_auth();
        Self::require_initialized(&env);

        if goal_amount <= 0 {
            panic!("goal must be positive");
        }
        if deadline <= env.ledger().timestamp() {
            panic!("deadline must be in the future");
        }

        let max_target = Self::max_target_from_inputs(goal_amount, &stretch_goal_inputs);
        let milestones = Self::build_milestones(&env, &milestone_inputs, max_target);
        let stretch_goals = Self::build_stretch_goals(&env, &stretch_goal_inputs, goal_amount);

        let campaign_id: u64 = env.storage().instance().get(&DataKey::NextCampaignId).unwrap();
        env.storage()
            .instance()
            .set(&DataKey::NextCampaignId, &(campaign_id + 1));

        let campaign = Campaign {
            id: campaign_id,
            creator: creator.clone(),
            title,
            description,
            goal_amount,
            deadline,
            amount_raised: 0,
            amount_claimed: 0,
            refunded_amount: 0,
            status: CampaignStatus::Active,
            created_at: env.ledger().timestamp(),
            funded_at: None,
            completed_at: None,
            contributions: Map::new(&env),
            refunded_backers: Map::new(&env),
            backers: Vec::new(&env),
            contribution_history: Vec::new(&env),
            milestones,
            stretch_goals,
        };

        Self::save_campaign(&env, &campaign);

        env.events().publish(
            (Symbol::new(&env, "campaign"), Symbol::new(&env, "created")),
            (campaign_id, creator, goal_amount, deadline),
        );

        campaign_id
    }

    pub fn contribute(env: Env, campaign_id: u64, backer: Address, amount: i128) -> i128 {
        backer.require_auth();
        Self::require_initialized(&env);

        if amount <= 0 {
            panic!("contribution must be positive");
        }

        let mut campaign = Self::get_campaign_or_panic(&env, campaign_id);
        Self::refresh_campaign(&env, &mut campaign);

        if env.ledger().timestamp() > campaign.deadline {
            panic!("campaign deadline passed");
        }
        if campaign.status == CampaignStatus::Failed || campaign.status == CampaignStatus::Completed {
            panic!("campaign not accepting contributions");
        }

        token::Client::new(&env, &Self::token_address(&env)).transfer(
            &backer,
            &env.current_contract_address(),
            &amount,
        );

        let current_total = campaign.contributions.get(backer.clone()).unwrap_or(0);
        campaign
            .contributions
            .set(backer.clone(), current_total + amount);

        if !Self::contains_backer(&campaign.backers, &backer) {
            campaign.backers.push_back(backer.clone());
        }

        campaign.amount_raised += amount;
        campaign.contribution_history.push_back(ContributionRecord {
            backer: backer.clone(),
            amount,
            timestamp: env.ledger().timestamp(),
        });

        Self::mark_reached_stretch_goals(&env, &mut campaign);
        Self::refresh_campaign(&env, &mut campaign);
        Self::save_campaign(&env, &campaign);

        env.events().publish(
            (Symbol::new(&env, "campaign"), Symbol::new(&env, "contrib")),
            (campaign_id, backer.clone(), amount, campaign.amount_raised),
        );

        campaign.contributions.get(backer).unwrap_or(0)
    }

    pub fn approve_milestone(env: Env, admin: Address, campaign_id: u64, milestone_id: u32) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut campaign = Self::get_campaign_or_panic(&env, campaign_id);
        Self::refresh_campaign(&env, &mut campaign);

        if campaign.status != CampaignStatus::Successful {
            panic!("campaign not funded");
        }

        let index = Self::milestone_index(&campaign, milestone_id);
        let mut milestone = campaign.milestones.get(index).unwrap();
        if milestone.approved {
            panic!("milestone already approved");
        }
        milestone.approved = true;
        campaign.milestones.set(index, milestone.clone());

        Self::save_campaign(&env, &campaign);

        env.events().publish(
            (Symbol::new(&env, "campaign"), Symbol::new(&env, "approve")),
            (campaign_id, milestone_id, milestone.payout_amount),
        );
    }

    pub fn claim_milestone(env: Env, campaign_id: u64, creator: Address, milestone_id: u32) -> i128 {
        creator.require_auth();
        Self::require_initialized(&env);

        let mut campaign = Self::get_campaign_or_panic(&env, campaign_id);
        Self::refresh_campaign(&env, &mut campaign);

        if campaign.creator != creator {
            panic!("only creator can claim");
        }
        if campaign.status != CampaignStatus::Successful {
            panic!("campaign not funded");
        }

        let index = Self::milestone_index(&campaign, milestone_id);
        let mut milestone = campaign.milestones.get(index).unwrap();
        if !milestone.approved {
            panic!("milestone not approved");
        }
        if milestone.claimed {
            panic!("milestone already claimed");
        }
        if campaign.amount_claimed + milestone.payout_amount > campaign.amount_raised {
            panic!("insufficient funded balance");
        }

        let payout = milestone.payout_amount;
        token::Client::new(&env, &Self::token_address(&env)).transfer(
            &env.current_contract_address(),
            &creator,
            &payout,
        );

        milestone.claimed = true;
        milestone.claimed_at = Some(env.ledger().timestamp());
        campaign.milestones.set(index, milestone.clone());
        campaign.amount_claimed += payout;

        Self::refresh_campaign(&env, &mut campaign);
        Self::save_campaign(&env, &campaign);

        env.events().publish(
            (Symbol::new(&env, "campaign"), Symbol::new(&env, "claim")),
            (campaign_id, milestone_id, payout),
        );

        payout
    }

    pub fn refund(env: Env, campaign_id: u64, backer: Address) -> i128 {
        backer.require_auth();
        Self::require_initialized(&env);

        let mut campaign = Self::get_campaign_or_panic(&env, campaign_id);
        Self::refresh_campaign(&env, &mut campaign);

        if campaign.status != CampaignStatus::Failed {
            panic!("campaign not refundable");
        }

        let contribution = campaign.contributions.get(backer.clone()).unwrap_or(0);
        if contribution <= 0 {
            panic!("no contribution to refund");
        }
        if campaign.refunded_backers.get(backer.clone()).unwrap_or(false) {
            panic!("refund already claimed");
        }

        token::Client::new(&env, &Self::token_address(&env)).transfer(
            &env.current_contract_address(),
            &backer,
            &contribution,
        );

        campaign.refunded_backers.set(backer.clone(), true);
        campaign.refunded_amount += contribution;
        Self::save_campaign(&env, &campaign);

        env.events().publish(
            (Symbol::new(&env, "campaign"), Symbol::new(&env, "refund")),
            (campaign_id, backer, contribution),
        );

        contribution
    }

    pub fn get_campaign(env: Env, campaign_id: u64) -> Campaign {
        let mut campaign = Self::get_campaign_or_panic(&env, campaign_id);
        Self::refresh_campaign(&env, &mut campaign);
        Self::save_campaign(&env, &campaign);
        campaign
    }

    pub fn get_campaign_summary(env: Env, campaign_id: u64) -> CampaignSummary {
        let campaign = Self::get_campaign(env, campaign_id);
        CampaignSummary {
            id: campaign.id,
            creator: campaign.creator.clone(),
            goal_amount: campaign.goal_amount,
            deadline: campaign.deadline,
            amount_raised: campaign.amount_raised,
            amount_claimed: campaign.amount_claimed,
            refunded_amount: campaign.refunded_amount,
            available_balance: campaign.amount_raised - campaign.amount_claimed - campaign.refunded_amount,
            status: campaign.status,
            backer_count: campaign.backers.len(),
            contribution_count: campaign.contribution_history.len(),
            goal_reached: campaign.amount_raised >= campaign.goal_amount,
            next_stretch_goal: Self::next_stretch_goal(&campaign),
        }
    }

    pub fn get_campaign_status(env: Env, campaign_id: u64) -> CampaignStatus {
        Self::get_campaign(env, campaign_id).status
    }

    pub fn get_contribution_history(env: Env, campaign_id: u64) -> Vec<ContributionRecord> {
        Self::get_campaign(env, campaign_id).contribution_history
    }

    pub fn get_backers(env: Env, campaign_id: u64) -> Vec<Address> {
        Self::get_campaign(env, campaign_id).backers
    }

    pub fn get_backer_contribution(env: Env, campaign_id: u64, backer: Address) -> i128 {
        Self::get_campaign(env, campaign_id)
            .contributions
            .get(backer)
            .unwrap_or(0)
    }

    pub fn get_milestones(env: Env, campaign_id: u64) -> Vec<Milestone> {
        Self::get_campaign(env, campaign_id).milestones
    }

    pub fn get_stretch_goals(env: Env, campaign_id: u64) -> Vec<StretchGoal> {
        Self::get_campaign(env, campaign_id).stretch_goals
    }

    pub fn get_campaign_balance(env: Env, campaign_id: u64) -> i128 {
        let campaign = Self::get_campaign(env, campaign_id);
        campaign.amount_raised - campaign.amount_claimed - campaign.refunded_amount
    }

    pub fn get_campaign_count(env: Env) -> u64 {
        Self::require_initialized(&env);
        let next_id: u64 = env.storage().instance().get(&DataKey::NextCampaignId).unwrap();
        next_id - 1
    }

    fn require_initialized(env: &Env) {
        if !env.storage().instance().has(&DataKey::Admin) {
            panic!("contract not initialized");
        }
    }

    fn assert_admin(env: &Env, admin: &Address) {
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if stored_admin != *admin {
            panic!("unauthorized");
        }
    }

    fn token_address(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::Token).unwrap()
    }

    fn get_campaign_or_panic(env: &Env, campaign_id: u64) -> Campaign {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .expect("campaign not found")
    }

    fn save_campaign(env: &Env, campaign: &Campaign) {
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign.id), campaign);
    }

    fn build_milestones(env: &Env, inputs: &Vec<MilestoneInput>, max_target: i128) -> Vec<Milestone> {
        let mut milestones = Vec::new(env);
        let mut total = 0i128;

        for index in 0..inputs.len() {
            let input = inputs.get(index).unwrap();
            if input.payout_amount <= 0 {
                panic!("milestone payout must be positive");
            }
            total += input.payout_amount;
            if total > max_target {
                panic!("milestone payouts exceed funding plan");
            }

            milestones.push_back(Milestone {
                id: index + 1,
                name: input.name,
                payout_amount: input.payout_amount,
                approved: false,
                claimed: false,
                claimed_at: None,
            });
        }

        milestones
    }

    fn build_stretch_goals(env: &Env, inputs: &Vec<StretchGoalInput>, goal_amount: i128) -> Vec<StretchGoal> {
        let mut stretch_goals = Vec::new(env);
        let mut previous_target = goal_amount;

        for index in 0..inputs.len() {
            let input = inputs.get(index).unwrap();
            if input.target_amount <= previous_target {
                panic!("stretch goals must be ascending and above the base goal");
            }

            stretch_goals.push_back(StretchGoal {
                id: index + 1,
                target_amount: input.target_amount,
                description: input.description,
                reached: false,
                reached_at: None,
            });
            previous_target = input.target_amount;
        }

        stretch_goals
    }

    fn max_target_from_inputs(goal_amount: i128, stretch_goal_inputs: &Vec<StretchGoalInput>) -> i128 {
        let mut max_target = goal_amount;
        for index in 0..stretch_goal_inputs.len() {
            let input = stretch_goal_inputs.get(index).unwrap();
            if input.target_amount > max_target {
                max_target = input.target_amount;
            }
        }
        max_target
    }

    fn contains_backer(backers: &Vec<Address>, backer: &Address) -> bool {
        for index in 0..backers.len() {
            if backers.get(index).unwrap() == *backer {
                return true;
            }
        }
        false
    }

    fn mark_reached_stretch_goals(env: &Env, campaign: &mut Campaign) {
        for index in 0..campaign.stretch_goals.len() {
            let mut stretch_goal = campaign.stretch_goals.get(index).unwrap();
            if !stretch_goal.reached && campaign.amount_raised >= stretch_goal.target_amount {
                stretch_goal.reached = true;
                stretch_goal.reached_at = Some(env.ledger().timestamp());
                campaign.stretch_goals.set(index, stretch_goal);
            }
        }
    }

    fn milestone_index(campaign: &Campaign, milestone_id: u32) -> u32 {
        for index in 0..campaign.milestones.len() {
            if campaign.milestones.get(index).unwrap().id == milestone_id {
                return index;
            }
        }
        panic!("milestone not found")
    }

    fn next_stretch_goal(campaign: &Campaign) -> Option<StretchGoal> {
        for index in 0..campaign.stretch_goals.len() {
            let stretch_goal = campaign.stretch_goals.get(index).unwrap();
            if !stretch_goal.reached {
                return Some(stretch_goal);
            }
        }
        None
    }

    fn all_milestones_claimed(campaign: &Campaign) -> bool {
        for index in 0..campaign.milestones.len() {
            if !campaign.milestones.get(index).unwrap().claimed {
                return false;
            }
        }
        true
    }

    fn refresh_campaign(env: &Env, campaign: &mut Campaign) {
        let now = env.ledger().timestamp();

        if campaign.amount_raised >= campaign.goal_amount {
            if campaign.funded_at.is_none() {
                campaign.funded_at = Some(now);
            }

            if now > campaign.deadline
                && campaign.amount_claimed == campaign.amount_raised
                && Self::all_milestones_claimed(campaign)
            {
                campaign.status = CampaignStatus::Completed;
                if campaign.completed_at.is_none() {
                    campaign.completed_at = Some(now);
                }
            } else {
                campaign.status = CampaignStatus::Successful;
            }
            return;
        }

        if now > campaign.deadline {
            campaign.status = CampaignStatus::Failed;
            return;
        }

        campaign.status = CampaignStatus::Active;
    }
}


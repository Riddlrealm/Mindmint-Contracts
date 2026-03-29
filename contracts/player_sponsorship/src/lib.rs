#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, IntoVal, Symbol, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MilestoneCondition {
    PuzzlesSolved(u32),
    RankAtMost(u32),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    pub condition: MilestoneCondition,
    pub reward_amount: i128,
    pub claimed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SponsorshipDeal {
    pub id: u32,
    pub sponsor: Address,
    pub player: Address,
    pub token: Address,
    pub total_amount: i128,
    pub released: i128,
    pub milestones: Vec<Milestone>,
    pub cancelled: bool,
}

#[contracttype]
pub enum DataKey {
    Admin,
    ProofOfActivity,
    Leaderboard,
    DealCount,
    Deal(u32),
    PlayerDeals(Address),
}

#[contract]
pub struct PlayerSponsorshipContract;

#[contractimpl]
impl PlayerSponsorshipContract {
    pub fn initialize(env: Env, admin: Address, proof_of_activity: Address, leaderboard: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::ProofOfActivity, &proof_of_activity);
        env.storage()
            .instance()
            .set(&DataKey::Leaderboard, &leaderboard);
        env.storage().instance().set(&DataKey::DealCount, &0u32);
    }

    pub fn create_deal(
        env: Env,
        sponsor: Address,
        player: Address,
        token_address: Address,
        milestones: Vec<Milestone>,
        total_amount: i128,
    ) -> u32 {
        sponsor.require_auth();

        if total_amount <= 0 {
            panic!("Total amount must be positive");
        }
        if milestones.len() == 0 {
            panic!("Must supply milestones");
        }

        let mut sum: i128 = 0;
        for m in milestones.iter() {
            if m.reward_amount <= 0 {
                panic!("Milestone reward must be positive");
            }
            if m.claimed {
                panic!("Milestone cannot be pre-claimed");
            }
            sum += m.reward_amount;
        }
        if sum != total_amount {
            panic!("Milestone rewards must sum to total_amount");
        }

        let mut count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::DealCount)
            .unwrap_or(0);
        count += 1;

        token::Client::new(&env, &token_address).transfer(
            &sponsor,
            &env.current_contract_address(),
            &total_amount,
        );

        let deal = SponsorshipDeal {
            id: count,
            sponsor: sponsor.clone(),
            player: player.clone(),
            token: token_address,
            total_amount,
            released: 0,
            milestones: milestones.clone(),
            cancelled: false,
        };

        env.storage().persistent().set(&DataKey::Deal(count), &deal);
        env.storage().instance().set(&DataKey::DealCount, &count);

        let mut list: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerDeals(player.clone()))
            .unwrap_or(Vec::new(&env));
        list.push_back(count);
        env.storage()
            .persistent()
            .set(&DataKey::PlayerDeals(player), &list);

        count
    }

    pub fn claim_milestone(env: Env, player: Address, deal_id: u32, milestone_index: u32) {
        player.require_auth();

        let mut deal: SponsorshipDeal = env
            .storage()
            .persistent()
            .get(&DataKey::Deal(deal_id))
            .expect("Deal not found");

        if deal.cancelled {
            panic!("Deal is cancelled");
        }
        if deal.player != player {
            panic!("Only the player can claim milestones");
        }
        if milestone_index >= deal.milestones.len() {
            panic!("Invalid milestone index");
        }

        let mut milestone = deal.milestones.get(milestone_index).unwrap();
        if milestone.claimed {
            panic!("Milestone already claimed");
        }

        if !Self::condition_met(&env, &player, &milestone.condition) {
            panic!("Milestone condition not met");
        }

        milestone.claimed = true;
        deal.milestones.set(milestone_index, milestone.clone());
        deal.released += milestone.reward_amount;

        token::Client::new(&env, &deal.token).transfer(
            &env.current_contract_address(),
            &player,
            &milestone.reward_amount,
        );

        env.storage().persistent().set(&DataKey::Deal(deal_id), &deal);

        env.events().publish(
            (Symbol::new(&env, "MilestoneClaimed"),),
            (deal_id, player, milestone_index, milestone.reward_amount),
        );
    }

    pub fn cancel_deal(env: Env, sponsor: Address, deal_id: u32) {
        sponsor.require_auth();

        let mut deal: SponsorshipDeal = env
            .storage()
            .persistent()
            .get(&DataKey::Deal(deal_id))
            .expect("Deal not found");

        if deal.cancelled {
            panic!("Deal already cancelled");
        }
        if deal.sponsor != sponsor {
            panic!("Only sponsor can cancel");
        }

        let remaining = deal.total_amount - deal.released;
        if remaining > 0 {
            token::Client::new(&env, &deal.token).transfer(
                &env.current_contract_address(),
                &sponsor,
                &remaining,
            );
        }

        deal.cancelled = true;
        env.storage().persistent().set(&DataKey::Deal(deal_id), &deal);
    }

    pub fn get_deal(env: Env, deal_id: u32) -> Option<SponsorshipDeal> {
        env.storage().persistent().get(&DataKey::Deal(deal_id))
    }

    pub fn list_player_deals(env: Env, player: Address) -> Vec<u32> {
        let ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerDeals(player))
            .unwrap_or(Vec::new(&env));

        let mut active: Vec<u32> = Vec::new(&env);
        for id in ids.iter() {
            if let Some(deal) = env.storage().persistent().get::<DataKey, SponsorshipDeal>(&DataKey::Deal(id)) {
                if !deal.cancelled && deal.released < deal.total_amount {
                    active.push_back(id);
                }
            }
        }
        active
    }

    fn condition_met(env: &Env, player: &Address, cond: &MilestoneCondition) -> bool {
        match cond {
            MilestoneCondition::PuzzlesSolved(threshold) => {
                let proof: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::ProofOfActivity)
                    .expect("proof not set");
                let count: u32 = env.invoke_contract(
                    &proof,
                    &Symbol::new(env, "get_activity_count"),
                    soroban_sdk::vec![
                        env,
                        player.clone().into_val(env),
                        0u32.into_val(env),
                    ],
                );
                count >= *threshold
            }
            MilestoneCondition::RankAtMost(threshold) => {
                let lb: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::Leaderboard)
                    .expect("leaderboard not set");
                let rank: u32 = env.invoke_contract(
                    &lb,
                    &Symbol::new(env, "get_player_rank"),
                    soroban_sdk::vec![
                        env,
                        player.clone().into_val(env),
                        leaderboard_types::TimePeriod::AllTime.into_val(env),
                    ],
                );
                rank > 0 && rank <= *threshold
            }
        }
    }
}

mod leaderboard_types {
    use soroban_sdk::contracttype;

    #[contracttype]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(u32)]
    pub enum TimePeriod {
        Daily = 0,
        Weekly = 1,
        AllTime = 2,
    }
}

#[cfg(test)]
mod test;

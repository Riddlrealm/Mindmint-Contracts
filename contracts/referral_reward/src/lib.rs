//! Pure-Rust referral reward logic used by the Mindmint referral system.
//!
//! This crate intentionally has no chain dependency: it models referral
//! registration, one-time reward claims, per-referrer stats, and an optional
//! reward cap so the logic can be unit-tested without a Soroban host.

use std::collections::HashMap;

pub use types::ReferralRecord;

pub mod types;

pub struct ReferralContract {
    pub referrals: HashMap<String, ReferralRecord>, // key: referee
    pub stats: HashMap<String, (u64, u64)>,         // referrer -> (count, total earned)
    pub reward_amount_referrer: u64,
    pub reward_amount_referee: u64,
    pub max_reward_per_user: Option<u64>,
}

impl ReferralContract {
    pub fn new(referrer_reward: u64, referee_reward: u64) -> Self {
        Self {
            referrals: HashMap::new(),
            stats: HashMap::new(),
            reward_amount_referrer: referrer_reward,
            reward_amount_referee: referee_reward,
            max_reward_per_user: None,
        }
    }

    pub fn register_referral(&mut self, referrer: String, referee: String) -> Result<(), String> {
        if self.referrals.contains_key(&referee) {
            return Err("Referral already registered".into());
        }

        let record = ReferralRecord {
            referrer: referrer.clone(),
            referee: referee.clone(),
            rewarded_at: None,
            reward_amount_referrer: self.reward_amount_referrer,
            reward_amount_referee: self.reward_amount_referee,
        };

        self.referrals.insert(referee, record);
        Ok(())
    }

    pub fn claim_referral_reward(&mut self, referee: String, now: u64) -> Result<(), String> {
        let record = self
            .referrals
            .get_mut(&referee)
            .ok_or("Referral not found")?;

        if record.rewarded_at.is_some() {
            return Err("Reward already claimed".into());
        }

        let referrer = record.referrer.clone();
        let referrer_amount = record.reward_amount_referrer;
        let referee_amount = record.reward_amount_referee;

        {
            let entry = self.stats.entry(referrer.clone()).or_insert((0, 0));
            let projected_total = entry.1 + referrer_amount;

            if let Some(cap) = self.max_reward_per_user {
                if projected_total > cap {
                    return Err("Reward cap exceeded".into());
                }
            }

            entry.0 += 1;
            entry.1 += referrer_amount;
        }

        // Transfer tokens (mocked here)
        self.transfer(&referrer, referrer_amount)?;
        self.transfer(&referee, referee_amount)?;

        if let Some(record) = self.referrals.get_mut(&referee) {
            record.rewarded_at = Some(now);
        }

        self.emit_referral_rewarded(&referrer, &referee, referrer_amount + referee_amount);

        Ok(())
    }

    fn transfer(&self, _to: &String, _amount: u64) -> Result<(), String> {
        // integrate with token runtime
        Ok(())
    }

    fn emit_referral_rewarded(&self, referrer: &String, referee: &String, amount: u64) {
        println!(
            "ReferralRewarded: referrer={}, referee={}, amount={}",
            referrer, referee, amount
        );
    }

    pub fn update_reward_amounts(&mut self, referrer_amount: u64, referee_amount: u64) {
        self.reward_amount_referrer = referrer_amount;
        self.reward_amount_referee = referee_amount;
    }

    pub fn get_referral_stats(&self, referrer: String) -> (u64, u64) {
        self.stats.get(&referrer).cloned().unwrap_or((0, 0))
    }

    pub fn update_reward_cap(&mut self, cap: Option<u64>) {
        self.max_reward_per_user = cap;
        println!("RewardCapUpdated: new_cap={:?}", cap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(referrer: &str, referee: &str, r: u64, e: u64) -> ReferralRecord {
        ReferralRecord {
            referrer: referrer.to_string(),
            referee: referee.to_string(),
            rewarded_at: None,
            reward_amount_referrer: r,
            reward_amount_referee: e,
        }
    }

    #[test]
    fn test_valid_claim() {
        let mut contract = ReferralContract::new(50, 25);
        contract
            .register_referral("alice".into(), "bob".into())
            .unwrap();
        assert!(contract.claim_referral_reward("bob".into(), 123456).is_ok());
    }

    #[test]
    fn test_duplicate_claim_rejected() {
        let mut contract = ReferralContract::new(50, 25);
        contract
            .register_referral("alice".into(), "bob".into())
            .unwrap();
        contract
            .claim_referral_reward("bob".into(), 123456)
            .unwrap();
        assert!(contract
            .claim_referral_reward("bob".into(), 123457)
            .is_err());
    }

    #[test]
    fn test_stats_accuracy() {
        let mut contract = ReferralContract::new(50, 25);
        contract
            .register_referral("alice".into(), "bob".into())
            .unwrap();
        contract
            .claim_referral_reward("bob".into(), 123456)
            .unwrap();
        let stats = contract.get_referral_stats("alice".into());
        assert_eq!(stats.0, 1);
        assert_eq!(stats.1, 50);
    }

    #[test]
    fn test_reward_cap_exceeded() {
        let mut contract = ReferralContract::new(50, 10);
        contract.update_reward_cap(Some(80));

        let referrer = "alice".to_string();
        contract
            .referrals
            .insert("user1".to_string(), record(&referrer, "user1", 50, 10));
        contract
            .referrals
            .insert("user2".to_string(), record(&referrer, "user2", 50, 10));

        // First claim should succeed (50 <= 80)
        assert!(contract.claim_referral_reward("user1".into(), 100).is_ok());

        // Second claim should fail (50 + 50 = 100 > 80)
        let result = contract.claim_referral_reward("user2".into(), 101);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Reward cap exceeded");
    }

    #[test]
    fn test_cap_update_scenario() {
        let mut contract = ReferralContract::new(50, 10);
        let referrer = "alice".to_string();
        contract
            .referrals
            .insert("user1".to_string(), record(&referrer, "user1", 50, 10));
        contract
            .referrals
            .insert("user2".to_string(), record(&referrer, "user2", 50, 10));

        contract.update_reward_cap(Some(50));
        assert!(contract.claim_referral_reward("user1".into(), 100).is_ok());

        // Fails due to cap
        assert!(contract
            .claim_referral_reward("user2".to_string(), 101)
            .is_err());

        // Increase cap
        contract.update_reward_cap(Some(100));
        assert!(contract.claim_referral_reward("user2".into(), 102).is_ok());
    }
}

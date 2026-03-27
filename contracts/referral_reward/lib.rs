use std::collections::HashMap;

pub struct ReferralContract {
    pub referrals: HashMap<String, ReferralRecord>, // key: referee
    pub stats: HashMap<String, (u64, u64)>, // referrer -> (count, total earned)
    pub reward_amount_referrer: u64,
    pub reward_amount_referee: u64,
}


impl ReferralContract {
    pub fn new(referrer_reward: u64, referee_reward: u64) -> Self {
        Self {
            referrals: HashMap::new(),
            stats: HashMap::new(),
            reward_amount_referrer: referrer_reward,
            reward_amount_referee: referee_reward,
        }
    }
}

impl ReferralContract {
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
}

impl ReferralContract {
    pub fn claim_referral_reward(&mut self, referee: String, now: u64) -> Result<(), String> {
        let record = self.referrals.get_mut(&referee).ok_or("Referral not found")?;

        if record.rewarded_at.is_some() {
            return Err("Reward already claimed".into());
        }

        // Transfer tokens (mocked here)
        self.transfer(&record.referrer, record.reward_amount_referrer)?;
        self.transfer(&record.referee, record.reward_amount_referee)?;

        record.rewarded_at = Some(now);

        // Update stats
        let entry = self.stats.entry(record.referrer.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += record.reward_amount_referrer;

        // Emit event
        self.emit_referral_rewarded(&record.referrer, &record.referee, record.reward_amount_referrer + record.reward_amount_referee);

        Ok(())
    }

    fn transfer(&self, _to: &String, _amount: u64) -> Result<(), String> {
        // integrate with token runtime
        Ok(())
    }

    fn emit_referral_rewarded(&self, referrer: &String, referee: &String, amount: u64) {
        println!("ReferralRewarded: referrer={}, referee={}, amount={}", referrer, referee, amount);
    }
}

impl ReferralContract {
    pub fn update_reward_amounts(&mut self, referrer_amount: u64, referee_amount: u64) {
        self.reward_amount_referrer = referrer_amount;
        self.reward_amount_referee = referee_amount;
    }
}

impl ReferralContract {
    pub fn get_referral_stats(&self, referrer: String) -> (u64, u64) {
        self.stats.get(&referrer).cloned().unwrap_or((0, 0))
    }
}

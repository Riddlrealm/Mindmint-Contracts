#[derive(Debug, Clone)]
pub struct ReferralRecord {
    pub referrer: String,
    pub referee: String,
    pub rewarded_at: Option<u64>, // timestamp when reward claimed
    pub reward_amount_referrer: u64,
    pub reward_amount_referee: u64,
}

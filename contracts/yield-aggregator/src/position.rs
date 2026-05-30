#[derive(scale::Encode, scale::Decode, Clone, scale_info::TypeInfo)]
pub struct AggregatorPosition {
    pub depositor: ink::primitives::AccountId,
    pub total_deposited: u128,
    pub current_strategy_id: u32,
    pub last_rebalance: u64,
    pub total_earned: u128,
}

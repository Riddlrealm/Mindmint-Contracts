#[derive(scale::Encode, scale::Decode, Clone, scale_info::TypeInfo)]
pub enum StrategyType {
    Staking,
    LpMining,
    Vault,
}

#[derive(scale::Encode, scale::Decode, Clone, scale_info::TypeInfo)]
pub struct Strategy {
    pub id: u32,
    pub contract_address: ink::primitives::AccountId,
    pub strategy_type: StrategyType,
    pub current_apy_bps: u32,
    pub total_deposited: u128,
}

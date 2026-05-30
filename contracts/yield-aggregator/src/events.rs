#[ink::event]
pub struct Deposited {
    #[ink(topic)]
    pub depositor: ink::primitives::AccountId,
    pub amount: u128,
    pub strategy_id: u32,
}

#[ink::event]
pub struct Withdrawn {
    #[ink(topic)]
    pub depositor: ink::primitives::AccountId,
    pub amount: u128,
    pub strategy_id: u32,
}

#[ink::event]
pub struct Rebalanced {
    #[ink(topic)]
    pub depositor: ink::primitives::AccountId,
    pub from_strategy: u32,
    pub to_strategy: u32,
}

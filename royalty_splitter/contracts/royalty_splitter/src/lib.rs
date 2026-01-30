#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, Map,
};

const BPS_DENOMINATOR: u32 = 10_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RoyaltyError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidPercentages = 4,
    BelowMinimumThreshold = 5,
    NoBalance = 6,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Initialized,
    Splits,
    Balances,
    TotalReceived,
    MinThreshold,
}

#[contract]
pub struct RoyaltySplitter;

#[contractimpl]
impl RoyaltySplitter {
    /* ================= INIT ================= */

    pub fn init(
        env: Env,
        admin: Address,
        splits: Map<Address, u32>,
        min_threshold: i128,
    ) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("Already initialized");
        }

        let mut total: u32 = 0;
        for (_, pct) in splits.iter() {
            total += pct;
        }

        if total != BPS_DENOMINATOR {
            panic!("Percentages must sum to 100%");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Splits, &splits);
        env.storage().instance().set(&DataKey::Balances, &Map::<Address, i128>::new(&env));
        env.storage().instance().set(&DataKey::TotalReceived, &0i128);
        env.storage().instance().set(&DataKey::MinThreshold, &min_threshold);
        env.storage().instance().set(&DataKey::Initialized, &true);
    }

    /* ================= RECEIVE FUNDS ================= */

    pub fn distribute(env: Env, amount: i128) {
        Self::require_init(&env);

        let min = env.storage().instance().get::<_, i128>(&DataKey::MinThreshold).unwrap();
        if amount < min {
            panic!("Below minimum threshold");
        }

        let splits = env.storage().instance().get::<_, Map<Address, u32>>(&DataKey::Splits).unwrap();
        let mut balances =
            env.storage().instance().get::<_, Map<Address, i128>>(&DataKey::Balances).unwrap();

        for (recipient, pct) in splits.iter() {
            let share = amount * pct as i128 / BPS_DENOMINATOR as i128;
            let current = balances.get(recipient.clone()).unwrap_or(0);
            balances.set(recipient, current + share);
        }

        let total = env
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalReceived)
            .unwrap();
        env.storage().instance().set(&DataKey::TotalReceived, &(total + amount));
        env.storage().instance().set(&DataKey::Balances, &balances);
    }

    /* ================= WITHDRAW ================= */

    pub fn withdraw(env: Env, recipient: Address) {
        Self::require_init(&env);

        let mut balances =
            env.storage().instance().get::<_, Map<Address, i128>>(&DataKey::Balances).unwrap();
        let amount = balances.get(recipient.clone()).unwrap_or(0);

        if amount <= 0 {
            panic!("No balance");
        }

        balances.set(recipient.clone(), 0);
        env.storage().instance().set(&DataKey::Balances, &balances);

        recipient.require_auth();
        // actual transfer handled by calling contract or off-chain escrow
    }

    /* ================= UPDATE SPLITS ================= */

    pub fn update_splits(env: Env, new_splits: Map<Address, u32>) {
        Self::require_admin(&env);

        let mut total = 0u32;
        for (_, pct) in new_splits.iter() {
            total += pct;
        }

        if total != BPS_DENOMINATOR {
            panic!("Invalid percentages");
        }

        env.storage().instance().set(&DataKey::Splits, &new_splits);
    }

    /* ================= EMERGENCY ================= */

    pub fn emergency_withdraw(env: Env, recipient: Address, amount: i128) {
        Self::require_admin(&env);

        let mut balances =
            env.storage().instance().get::<_, Map<Address, i128>>(&DataKey::Balances).unwrap();
        let current = balances.get(recipient.clone()).unwrap_or(0);

        if amount > current {
            panic!("Insufficient funds");
        }

        balances.set(recipient, current - amount);
        env.storage().instance().set(&DataKey::Balances, &balances);
    }

    /* ================= HELPERS ================= */

    fn require_init(env: &Env) {
        if !env
            .storage()
            .instance()
            .has(&DataKey::Initialized)
        {
            panic!("Not initialized");
        }
    }

    fn require_admin(env: &Env) {
        let admin = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .unwrap();
        admin.require_auth();
    }
}

mod test;
use soroban_sdk::{Env, Address, symbol_short};

pub(crate) fn emit_pool_initialized(
    env: &Env,
    staking_token: Address,
    reward_token: Address,
    apy_bps: u32,
    lockup_period: u64,
) {
    env.events().publish(
        (symbol_short!("init"), staking_token, reward_token),
        (apy_bps, lockup_period),
    );
}

pub(crate) fn emit_staked(
    env: &Env,
    user: Address,
    amount: i128,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("stake"), user),
        (amount, timestamp),
    );
}

pub(crate) fn emit_unstaked(
    env: &Env,
    user: Address,
    amount: i128,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("unstake"), user),
        (amount, timestamp),
    );
}

pub(crate) fn emit_rewards_claimed(
    env: &Env,
    user: Address,
    amount: i128,
    timestamp: u64,
    auto_compounded: bool,
) {
    env.events().publish(
        (symbol_short!("claim"), user),
        (amount, timestamp, auto_compounded),
    );
}

pub(crate) fn emit_apy_updated(
    env: &Env,
    new_apy_bps: u32,
) {
    env.events().publish(
        (symbol_short!("apy_update"),),
        new_apy_bps,
    );
}

pub(crate) fn emit_pool_funded(
    env: &Env,
    amount: i128,
) {
    env.events().publish(
        (symbol_short!("fund"),),
        amount,
    );
}

pub(crate) fn emit_auto_compound_toggled(
    env: &Env,
    user: Address,
    enabled: bool,
) {
    env.events().publish(
        (symbol_short!("autocompound"), user),
        enabled,
    );
}
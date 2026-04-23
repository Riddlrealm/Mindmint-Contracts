#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Symbol,
};

const BASIS_POINTS: i128 = 10_000;

#[contracttype]
#[derive(Clone, Debug)]
pub enum DataKey {
    Config,
    Pool(u32),
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Config {
    pub admin: Address,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct LiquidityPool {
    pub token_a: Address,
    pub token_b: Address,
    pub reserve_a: i128,
    pub reserve_b: i128,
    pub fee_bps: u32,
    pub total_swaps: u64,
    pub fees_a: i128,
    pub fees_b: i128,
}

#[contract]
pub struct TokenSwapContract;

#[contractimpl]
impl TokenSwapContract {
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        if env.storage().persistent().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        env.storage().persistent().set(&DataKey::Config, &Config { admin });
    }

    pub fn create_pool(env: Env, admin: Address, pool_id: u32, token_a: Address, token_b: Address, fee_bps: u32) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if token_a == token_b {
            panic!("Tokens must be different");
        }
        if fee_bps > BASIS_POINTS as u32 {
            panic!("Invalid fee");
        }
        if env.storage().persistent().has(&DataKey::Pool(pool_id)) {
            panic!("Pool already exists");
        }

        let pool = LiquidityPool {
            token_a,
            token_b,
            reserve_a: 0,
            reserve_b: 0,
            fee_bps,
            total_swaps: 0,
            fees_a: 0,
            fees_b: 0,
        };

        env.storage().persistent().set(&DataKey::Pool(pool_id), &pool);
    }

    pub fn add_liquidity(env: Env, admin: Address, pool_id: u32, amount_a: i128, amount_b: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        if amount_a <= 0 || amount_b <= 0 {
            panic!("Amounts must be positive");
        }

        let mut pool = Self::must_get_pool(&env, pool_id);

        let token_a_client = token::Client::new(&env, &pool.token_a);
        let token_b_client = token::Client::new(&env, &pool.token_b);

        token_a_client.transfer(&admin, &env.current_contract_address(), &amount_a);
        token_b_client.transfer(&admin, &env.current_contract_address(), &amount_b);

        pool.reserve_a += amount_a;
        pool.reserve_b += amount_b;

        env.storage().persistent().set(&DataKey::Pool(pool_id), &pool);

        env.events().publish(
            (Symbol::new(&env, "LiquidityAdded"), pool_id),
            (amount_a, amount_b),
        );
    }

    pub fn quote_swap(env: Env, pool_id: u32, token_in: Address, amount_in: i128) -> i128 {
        if amount_in <= 0 {
            panic!("Amount must be positive");
        }
        let pool = Self::must_get_pool(&env, pool_id);
        let (reserve_in, reserve_out, _) = Self::reserves_for_input(&pool, &token_in);

        let gross_out = Self::quote_out_amount(reserve_in, reserve_out, amount_in);
        let fee = (gross_out * pool.fee_bps as i128) / BASIS_POINTS;
        gross_out - fee
    }

    pub fn swap(env: Env, player: Address, pool_id: u32, token_in: Address, amount_in: i128) -> i128 {
        player.require_auth();
        if amount_in <= 0 {
            panic!("Amount must be positive");
        }

        let mut pool = Self::must_get_pool(&env, pool_id);
        let contract_addr = env.current_contract_address();

        let (reserve_in, reserve_out, input_is_a) = Self::reserves_for_input(&pool, &token_in);
        if reserve_in <= 0 || reserve_out <= 0 {
            panic!("Insufficient liquidity");
        }

        let amount_out_gross = Self::quote_out_amount(reserve_in, reserve_out, amount_in);
        if amount_out_gross <= 0 {
            panic!("Insufficient output");
        }

        let fee = (amount_out_gross * pool.fee_bps as i128) / BASIS_POINTS;
        let amount_out = amount_out_gross - fee;
        if amount_out <= 0 {
            panic!("Output too small");
        }

        if input_is_a {
            if amount_out_gross > pool.reserve_b {
                panic!("Insufficient liquidity");
            }

            let token_a_client = token::Client::new(&env, &pool.token_a);
            let token_b_client = token::Client::new(&env, &pool.token_b);

            token_a_client.transfer(&player, &contract_addr, &amount_in);
            token_b_client.transfer(&contract_addr, &player, &amount_out);

            pool.reserve_a += amount_in;
            pool.reserve_b -= amount_out_gross;
            pool.fees_b += fee;

            env.events().publish(
                (Symbol::new(&env, "Swapped"), pool_id),
                (player, pool.token_a.clone(), amount_in, pool.token_b.clone(), amount_out),
            );
        } else {
            if amount_out_gross > pool.reserve_a {
                panic!("Insufficient liquidity");
            }

            let token_a_client = token::Client::new(&env, &pool.token_a);
            let token_b_client = token::Client::new(&env, &pool.token_b);

            token_b_client.transfer(&player, &contract_addr, &amount_in);
            token_a_client.transfer(&contract_addr, &player, &amount_out);

            pool.reserve_b += amount_in;
            pool.reserve_a -= amount_out_gross;
            pool.fees_a += fee;

            env.events().publish(
                (Symbol::new(&env, "Swapped"), pool_id),
                (player, pool.token_b.clone(), amount_in, pool.token_a.clone(), amount_out),
            );
        }

        pool.total_swaps += 1;
        env.storage().persistent().set(&DataKey::Pool(pool_id), &pool);

        amount_out
    }

    pub fn remove_liquidity(env: Env, admin: Address, pool_id: u32, amount_a: i128, amount_b: i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        if amount_a < 0 || amount_b < 0 {
            panic!("Amounts cannot be negative");
        }

        let mut pool = Self::must_get_pool(&env, pool_id);

        let available_a = pool.reserve_a - pool.fees_a;
        let available_b = pool.reserve_b - pool.fees_b;

        if amount_a > available_a || amount_b > available_b {
            panic!("Insufficient liquidity");
        }

        let token_a_client = token::Client::new(&env, &pool.token_a);
        let token_b_client = token::Client::new(&env, &pool.token_b);

        if amount_a > 0 {
            token_a_client.transfer(&env.current_contract_address(), &admin, &amount_a);
            pool.reserve_a -= amount_a;
        }
        if amount_b > 0 {
            token_b_client.transfer(&env.current_contract_address(), &admin, &amount_b);
            pool.reserve_b -= amount_b;
        }

        env.storage().persistent().set(&DataKey::Pool(pool_id), &pool);
    }

    pub fn claim_fees(env: Env, admin: Address, pool_id: u32) -> (i128, i128) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut pool = Self::must_get_pool(&env, pool_id);
        let fees_a = pool.fees_a;
        let fees_b = pool.fees_b;

        if fees_a == 0 && fees_b == 0 {
            return (0, 0);
        }

        let token_a_client = token::Client::new(&env, &pool.token_a);
        let token_b_client = token::Client::new(&env, &pool.token_b);

        if fees_a > 0 {
            token_a_client.transfer(&env.current_contract_address(), &admin, &fees_a);
            pool.reserve_a -= fees_a;
            pool.fees_a = 0;
        }
        if fees_b > 0 {
            token_b_client.transfer(&env.current_contract_address(), &admin, &fees_b);
            pool.reserve_b -= fees_b;
            pool.fees_b = 0;
        }

        env.storage().persistent().set(&DataKey::Pool(pool_id), &pool);

        env.events().publish(
            (Symbol::new(&env, "FeesClaimed"), pool_id),
            (fees_a, fees_b),
        );

        (fees_a, fees_b)
    }

    pub fn get_pool(env: Env, pool_id: u32) -> LiquidityPool {
        Self::must_get_pool(&env, pool_id)
    }

    fn must_get_pool(env: &Env, pool_id: u32) -> LiquidityPool {
        env.storage()
            .persistent()
            .get(&DataKey::Pool(pool_id))
            .unwrap_or_else(|| panic!("Pool not found"))
    }

    fn get_config(env: &Env) -> Config {
        env.storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| panic!("Not initialized"))
    }

    fn assert_admin(env: &Env, user: &Address) {
        let cfg = Self::get_config(env);
        if cfg.admin != *user {
            panic!("Admin only");
        }
    }

    fn reserves_for_input(pool: &LiquidityPool, token_in: &Address) -> (i128, i128, bool) {
        if *token_in == pool.token_a {
            (pool.reserve_a, pool.reserve_b, true)
        } else if *token_in == pool.token_b {
            (pool.reserve_b, pool.reserve_a, false)
        } else {
            panic!("Token not in pool");
        }
    }

    fn quote_out_amount(reserve_in: i128, reserve_out: i128, amount_in: i128) -> i128 {
        // constant product: out = (reserve_out * amount_in) / (reserve_in + amount_in)
        // (integer math, rounds down)
        (reserve_out * amount_in) / (reserve_in + amount_in)
    }
}

#[cfg(test)]
mod test;

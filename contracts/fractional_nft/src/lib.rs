#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, vec, Address, Env, IntoVal, Symbol,
    Val,
};

const VOTING_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;

// ================= TYPES =================

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultStatus {
    Active = 0,
    BuyoutPending = 1,
    Completed = 2,
    Rejected = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalVault {
    pub id: u64,
    pub nft_contract: Address,
    pub nft_id: u32,
    pub owner: Address,
    pub total_fractions: i128,
    pub fraction_token: Address,
    pub status: VaultStatus,
    pub buyout_price: i128,
    pub payment_token: Address,
    pub buyout_votes_for: i128,
    pub buyout_votes_against: i128,
    pub buyout_buyer: Option<Address>,
    pub buyout_offer_price: i128,
    pub buyout_deadline: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    VaultCount,
    Vault(u64),
    Balance(u64, Address),
    Voted(u64, Address),
}

// ================= CONTRACT =================

#[contract]
pub struct FractionalNftContract;

#[contractimpl]
impl FractionalNftContract {

    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already_initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::VaultCount, &0u64);
    }

    pub fn fractionalize(
        env: Env,
        owner: Address,
        nft_contract: Address,
        nft_id: u32,
        total_fractions: i128,
        buyout_price: i128,
        payment_token: Address,
    ) -> u64 {
        owner.require_auth();

        // Fix: invoke_contract requires Vec<Val>, not a tuple.
        // Each argument must be converted with .into_val(&env).
        let owner_of_args: soroban_sdk::Vec<Val> = vec![&env, nft_id.into_val(&env)];
        let current_owner: Address = env.invoke_contract(
            &nft_contract,
            &Symbol::new(&env, "owner_of"),
            owner_of_args,
        );

        if current_owner != owner {
            panic!("not_nft_owner");
        }

        let transfer_args: soroban_sdk::Vec<Val> = vec![
            &env,
            owner.clone().into_val(&env),
            env.current_contract_address().into_val(&env),
            nft_id.into_val(&env),
        ];
        env.invoke_contract::<()>(
            &nft_contract,
            &Symbol::new(&env, "transfer"),
            transfer_args,
        );

        let id = Self::next_vault_id(&env);

        let vault = FractionalVault {
            id,
            nft_contract,
            nft_id,
            owner: owner.clone(),
            total_fractions,
            fraction_token: env.current_contract_address(),
            status: VaultStatus::Active,
            buyout_price,
            payment_token,
            buyout_votes_for: 0,
            buyout_votes_against: 0,
            buyout_buyer: None,
            buyout_offer_price: 0,
            buyout_deadline: 0,
        };

        env.storage().persistent().set(&DataKey::Vault(id), &vault);

        Self::set_balance(&env, id, &owner, total_fractions);

        env.events().publish(
            (symbol_short!("NFTFrac"), id),
            (owner, nft_id, total_fractions, buyout_price),
        );

        id
    }

    pub fn initiate_buyout(env: Env, buyer: Address, vault_id: u64, offer_price: i128) {
        buyer.require_auth();

        let mut vault = Self::require_active_vault(&env, vault_id);

        let token_client = token::Client::new(&env, &vault.payment_token);
        token_client.transfer(&buyer, &env.current_contract_address(), &offer_price);

        let deadline = env.ledger().timestamp() + VOTING_PERIOD_SECS;

        vault.status = VaultStatus::BuyoutPending;
        vault.buyout_buyer = Some(buyer.clone());
        vault.buyout_offer_price = offer_price;
        vault.buyout_deadline = deadline;

        env.storage().persistent().set(&DataKey::Vault(vault_id), &vault);

        env.events().publish(
            (symbol_short!("BuyoutIn"), vault_id),
            (buyer, offer_price, deadline),
        );
    }

    fn next_vault_id(env: &Env) -> u64 {
        let mut id: u64 = env.storage().instance().get(&DataKey::VaultCount).unwrap_or(0);
        id += 1;
        env.storage().instance().set(&DataKey::VaultCount, &id);
        id
    }

    fn require_active_vault(env: &Env, vault_id: u64) -> FractionalVault {
        let vault: FractionalVault = env
            .storage()
            .persistent()
            .get(&DataKey::Vault(vault_id))
            .unwrap_or_else(|| panic!("vault_not_found"));

        if vault.status != VaultStatus::Active {
            panic!("vault_not_active");
        }

        vault
    }

    fn get_balance(env: &Env, vault_id: u64, owner: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(vault_id, owner.clone()))
            .unwrap_or(0)
    }

    fn set_balance(env: &Env, vault_id: u64, owner: &Address, amount: i128) {
        env.storage()
            .persistent()
            .set(&DataKey::Balance(vault_id, owner.clone()), &amount);
    }
}

mod test;
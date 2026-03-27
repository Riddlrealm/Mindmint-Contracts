#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, IntoVal, Symbol,
};

/// Voting window after a buyout is initiated (7 days in seconds).
const VOTING_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VaultStatus {
    /// NFT is locked; fractions circulate freely.
    Active = 0,
    /// A buyout offer is open and fraction holders are voting.
    BuyoutPending = 1,
    /// Buyout vote passed; NFT has been transferred to the buyer.
    Completed = 2,
    /// Buyout vote failed or deadline lapsed; offer refunded.
    Rejected = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalVault {
    /// Unique vault identifier.
    pub id: u64,
    /// Address of the NFT contract.
    pub nft_contract: Address,
    /// Token ID of the locked NFT.
    pub nft_id: u32,
    /// Original fractionalizer (receives fraction-sale proceeds).
    pub owner: Address,
    /// Total fraction supply minted for this vault.
    pub total_fractions: i128,
    /// Address that manages fraction balances (this contract).
    pub fraction_token: Address,
    /// Current lifecycle state of the vault.
    pub status: VaultStatus,
    /// Minimum total offer required to trigger a buyout vote.
    pub buyout_price: i128,
    /// Fungible token used for fraction purchases and buyout offers.
    pub payment_token: Address,
    /// Weighted votes in favour of the current buyout offer.
    pub buyout_votes_for: i128,
    /// Weighted votes against the current buyout offer.
    pub buyout_votes_against: i128,
    /// Address of the prospective buyer during a pending buyout.
    pub buyout_buyer: Option<Address>,
    /// Escrowed offer amount for the pending buyout.
    pub buyout_offer_price: i128,
    /// UNIX timestamp after which the vote can be settled.
    pub buyout_deadline: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    VaultCount,
    /// Per-vault storage of the `FractionalVault` struct.
    Vault(u64),
    /// Fraction balance: (vault_id, holder) → i128.
    Balance(u64, Address),
    /// Vote record: (vault_id, voter) → bool (prevents double-voting).
    Voted(u64, Address),
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct FractionalNftContract;

#[contractimpl]
impl FractionalNftContract {
    // -----------------------------------------------------------------------
    // Initialisation
    // -----------------------------------------------------------------------

    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already_initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::VaultCount, &0u64);
    }

    // -----------------------------------------------------------------------
    // Core: fractionalize
    // -----------------------------------------------------------------------

    /// Lock `nft_id` from `nft_contract`, mint `total_fractions` to `owner`,
    /// and record the minimum `buyout_price` denominated in `payment_token`.
    /// Returns the new `vault_id`.
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

        if total_fractions <= 0 {
            panic!("invalid_total_fractions");
        }
        if buyout_price <= 0 {
            panic!("invalid_buyout_price");
        }

        // Verify the caller owns the NFT.
        let owner_of_args = (nft_id,).into_val(&env);
        let current_owner: Address = env.invoke_contract(
            &nft_contract,
            &Symbol::new(&env, "owner_of"),
            owner_of_args,
        );
        if current_owner != owner {
            panic!("not_nft_owner");
        }

        // Transfer NFT into this contract (lock it).
        let transfer_args =
            (owner.clone(), env.current_contract_address(), nft_id).into_val(&env);
        env.invoke_contract::<()>(&nft_contract, &Symbol::new(&env, "transfer"), transfer_args);

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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Vault(id), 100_000, 500_000);

        // Mint all fractions to the original owner.
        Self::set_balance(&env, id, &owner, total_fractions);

        // Event: NFTFractionalized
        env.events().publish(
            (symbol_short!("NFTFrac"), id),
            (owner, nft_id, total_fractions, buyout_price),
        );

        id
    }

    // -----------------------------------------------------------------------
    // Fraction trading
    // -----------------------------------------------------------------------

    /// Buy `amount` fractions from the vault owner at the pro-rata price
    /// derived from `buyout_price / total_fractions`.  Payment flows directly
    /// to the original `owner` of the vault.
    pub fn buy_fraction(env: Env, buyer: Address, vault_id: u64, amount: i128) {
        buyer.require_auth();

        if amount <= 0 {
            panic!("invalid_amount");
        }

        let vault = Self::require_active_vault(&env, vault_id);

        // Only fractions still held by the original owner are available for
        // this fixed-price purchase path.
        let owner_balance = Self::get_balance(&env, vault_id, &vault.owner);
        if owner_balance < amount {
            panic!("insufficient_fractions_available");
        }

        // Price per fraction (integer division; any remainder stays with seller).
        let fraction_price = vault.buyout_price / vault.total_fractions;
        let total_cost = fraction_price
            .checked_mul(amount)
            .unwrap_or_else(|| panic!("overflow"));

        // Transfer payment from buyer to vault owner.
        let token_client = token::Client::new(&env, &vault.payment_token);
        token_client.transfer(&buyer, &vault.owner, &total_cost);

        // Move fractions from owner → buyer.
        Self::set_balance(&env, vault_id, &vault.owner, owner_balance - amount);
        let buyer_balance = Self::get_balance(&env, vault_id, &buyer);
        Self::set_balance(&env, vault_id, &buyer, buyer_balance + amount);
    }

    /// Peer-to-peer fraction transfer (no payment routing; caller arranges
    /// settlement separately).
    pub fn transfer_fraction(env: Env, from: Address, vault_id: u64, to: Address, amount: i128) {
        from.require_auth();
        Self::require_active_vault(&env, vault_id);

        if amount <= 0 {
            panic!("invalid_amount");
        }
        if from == to {
            panic!("cannot_transfer_to_self");
        }

        let from_bal = Self::get_balance(&env, vault_id, &from);
        if from_bal < amount {
            panic!("insufficient_balance");
        }
        let to_bal = Self::get_balance(&env, vault_id, &to);

        Self::set_balance(&env, vault_id, &from, from_bal - amount);
        Self::set_balance(&env, vault_id, &to, to_bal + amount);
    }

    // -----------------------------------------------------------------------
    // Buyout: initiate → vote → settle → claim
    // -----------------------------------------------------------------------

    /// Propose a full buyout at `offer_price` (must be ≥ `buyout_price`).
    /// The offered amount is escrowed immediately and voting opens for
    /// `VOTING_PERIOD_SECS` seconds.
    pub fn initiate_buyout(env: Env, buyer: Address, vault_id: u64, offer_price: i128) {
        buyer.require_auth();

        let mut vault = Self::require_active_vault(&env, vault_id);

        if offer_price < vault.buyout_price {
            panic!("offer_below_buyout_price");
        }

        // Escrow the full offer from the buyer.
        let token_client = token::Client::new(&env, &vault.payment_token);
        token_client.transfer(&buyer, &env.current_contract_address(), &offer_price);

        let deadline = env.ledger().timestamp() + VOTING_PERIOD_SECS;

        vault.status = VaultStatus::BuyoutPending;
        vault.buyout_buyer = Some(buyer.clone());
        vault.buyout_offer_price = offer_price;
        vault.buyout_deadline = deadline;
        vault.buyout_votes_for = 0;
        vault.buyout_votes_against = 0;

        env.storage().persistent().set(&DataKey::Vault(vault_id), &vault);

        // Event: BuyoutInitiated
        env.events().publish(
            (symbol_short!("BuyoutIn"), vault_id),
            (buyer, offer_price, deadline),
        );
    }

    /// Cast a weighted vote on the active buyout.  `approve = true` to accept
    /// the offer, `false` to reject.  Voting power equals the caller's current
    /// fraction balance.  Each address may vote once per buyout.
    pub fn vote_buyout(env: Env, voter: Address, vault_id: u64, approve: bool) {
        voter.require_auth();

        let mut vault: FractionalVault = env
            .storage()
            .persistent()
            .get(&DataKey::Vault(vault_id))
            .unwrap_or_else(|| panic!("vault_not_found"));

        if vault.status != VaultStatus::BuyoutPending {
            panic!("no_active_buyout");
        }

        if env.ledger().timestamp() > vault.buyout_deadline {
            panic!("voting_ended");
        }

        if env
            .storage()
            .persistent()
            .has(&DataKey::Voted(vault_id, voter.clone()))
        {
            panic!("already_voted");
        }

        let weight = Self::get_balance(&env, vault_id, &voter);
        if weight <= 0 {
            panic!("no_fractions");
        }

        if approve {
            vault.buyout_votes_for += weight;
        } else {
            vault.buyout_votes_against += weight;
        }

        env.storage().persistent().set(&DataKey::Vault(vault_id), &vault);
        env.storage()
            .persistent()
            .set(&DataKey::Voted(vault_id, voter.clone()), &true);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Voted(vault_id, voter), 100_000, 500_000);
    }

    /// Settle the buyout after the voting deadline has passed.
    ///
    /// * **Vote passes** (for_votes > 50 % of total_fractions): NFT is
    ///   transferred to the buyer; proceeds remain escrowed for holders to
    ///   claim via `claim_proceeds`.
    ///
    /// * **Vote fails**: offer is refunded to buyer; vault reverts to Active.
    pub fn settle_buyout(env: Env, vault_id: u64) {
        let mut vault: FractionalVault = env
            .storage()
            .persistent()
            .get(&DataKey::Vault(vault_id))
            .unwrap_or_else(|| panic!("vault_not_found"));

        if vault.status != VaultStatus::BuyoutPending {
            panic!("no_active_buyout");
        }

        if env.ledger().timestamp() <= vault.buyout_deadline {
            panic!("voting_not_ended");
        }

        let buyer = vault.buyout_buyer.clone().unwrap();

        // >50 % of the total fraction supply must have voted in favour.
        let approved = vault.buyout_votes_for * 2 > vault.total_fractions;

        if approved {
            // Transfer the locked NFT to the buyer.
            let transfer_args =
                (env.current_contract_address(), buyer.clone(), vault.nft_id).into_val(&env);
            env.invoke_contract::<()>(
                &vault.nft_contract,
                &Symbol::new(&env, "transfer"),
                transfer_args,
            );

            vault.status = VaultStatus::Completed;
            env.storage().persistent().set(&DataKey::Vault(vault_id), &vault);

            // Event: BuyoutCompleted
            env.events().publish(
                (symbol_short!("BuyoutOk"), vault_id),
                (buyer, vault.buyout_offer_price),
            );
        } else {
            // Refund the escrowed offer to the buyer.
            let token_client = token::Client::new(&env, &vault.payment_token);
            token_client.transfer(
                &env.current_contract_address(),
                &buyer,
                &vault.buyout_offer_price,
            );

            // Reset buyout state; vault returns to Active.
            vault.status = VaultStatus::Active;
            vault.buyout_buyer = None;
            vault.buyout_offer_price = 0;
            vault.buyout_deadline = 0;
            vault.buyout_votes_for = 0;
            vault.buyout_votes_against = 0;
            env.storage().persistent().set(&DataKey::Vault(vault_id), &vault);

            // Event: BuyoutRejected
            env.events().publish((symbol_short!("BuyoutNo"), vault_id), ());
        }
    }

    /// After a completed buyout, fraction holders call this to redeem their
    /// pro-rata share of the escrowed proceeds.
    pub fn claim_proceeds(env: Env, holder: Address, vault_id: u64) {
        holder.require_auth();

        let vault: FractionalVault = env
            .storage()
            .persistent()
            .get(&DataKey::Vault(vault_id))
            .unwrap_or_else(|| panic!("vault_not_found"));

        if vault.status != VaultStatus::Completed {
            panic!("buyout_not_completed");
        }

        let fractions = Self::get_balance(&env, vault_id, &holder);
        if fractions <= 0 {
            panic!("no_fractions");
        }

        // Proportional share of the total offer.
        let payout = vault
            .buyout_offer_price
            .checked_mul(fractions)
            .unwrap_or_else(|| panic!("overflow"))
            / vault.total_fractions;

        // Burn the holder's fractions to prevent double-claiming.
        Self::set_balance(&env, vault_id, &holder, 0);

        let token_client = token::Client::new(&env, &vault.payment_token);
        token_client.transfer(&env.current_contract_address(), &holder, &payout);
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Return the full vault state (lock status, fraction supply, buyout info).
    pub fn get_vault(env: Env, vault_id: u64) -> Option<FractionalVault> {
        env.storage().persistent().get(&DataKey::Vault(vault_id))
    }

    /// Return the fraction balance of `owner` in `vault_id`.
    pub fn balance_of(env: Env, vault_id: u64, owner: Address) -> i128 {
        Self::get_balance(&env, vault_id, &owner)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn next_vault_id(env: &Env) -> u64 {
        let mut id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::VaultCount)
            .unwrap_or(0);
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Balance(vault_id, owner.clone()), 100_000, 500_000);
    }
}

mod test;

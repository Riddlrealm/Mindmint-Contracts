#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, IntoVal, Symbol,
};

const BASIS_POINTS: i128 = 10_000;

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetType {
    Token = 1,
    NFT = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    pub asset_type: AssetType,
    pub contract: Address,
    pub amount: i128, // Token amount or 1 for NFT
    pub nft_id: u32,  // Only used for NFT assets
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanTerms {
    pub duration_secs: u64,
    pub interest_bps: u32,
    pub max_extension_secs: u64,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfferStatus {
    Open = 1,
    Cancelled = 2,
    Accepted = 3,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoanStatus {
    Active = 1,
    Repaid = 2,
    Liquidated = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanOffer {
    pub offer_id: u64,
    pub lender: Address,
    pub loan_asset: Asset,
    pub collateral_asset: Asset,
    pub repayment_token: Address,
    pub principal: i128,
    pub terms: LoanTerms,
    pub status: OfferStatus,
    pub created_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Loan {
    pub loan_id: u64,
    pub offer_id: u64,
    pub lender: Address,
    pub borrower: Address,
    pub loan_asset: Asset,
    pub collateral_asset: Asset,
    pub repayment_token: Address,
    pub principal: i128,
    pub outstanding_principal: i128,
    pub accrued_interest: i128,
    pub start_time: u64,
    pub due_time: u64,
    pub last_accrual_time: u64,
    pub status: LoanStatus,
    pub terms: LoanTerms,
    pub extension_secs: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionRequest {
    pub loan_id: u64,
    pub borrower: Address,
    pub requested_extension_secs: u64,
    pub requested_time: u64,
}

#[contracttype]
pub enum DataKey {
    Offer(u64),
    OfferCount,
    Loan(u64),
    LoanCount,
    ExtensionRequest(u64),
}

#[contract]
pub struct LendingContract;

#[contractimpl]
impl LendingContract {
    pub fn create_offer(
        env: Env,
        lender: Address,
        loan_asset: Asset,
        collateral_asset: Asset,
        repayment_token: Address,
        principal: i128,
        terms: LoanTerms,
    ) -> u64 {
        lender.require_auth();

        Self::validate_asset(&loan_asset);
        Self::validate_asset(&collateral_asset);

        if terms.duration_secs == 0 {
            panic!("Duration must be greater than zero");
        }
        if principal <= 0 {
            panic!("Principal must be greater than zero");
        }

        match loan_asset.asset_type {
            AssetType::Token => {
                if principal != loan_asset.amount {
                    panic!("Principal must match token loan amount");
                }
                if repayment_token != loan_asset.contract {
                    panic!("Repayment token must match loan token");
                }
            }
            AssetType::NFT => {
                if loan_asset.amount != 1 {
                    panic!("NFT amount must be 1");
                }
            }
        }

        let offer_id = Self::next_offer_id(&env);
        let created_time = env.ledger().timestamp();

        let offer = LoanOffer {
            offer_id,
            lender: lender.clone(),
            loan_asset: loan_asset.clone(),
            collateral_asset,
            repayment_token,
            principal,
            terms,
            status: OfferStatus::Open,
            created_time,
        };

        Self::transfer_asset(
            &env,
            &loan_asset,
            &lender,
            &env.current_contract_address(),
        );

        env.storage().persistent().set(&DataKey::Offer(offer_id), &offer);
        offer_id
    }

    pub fn cancel_offer(env: Env, lender: Address, offer_id: u64) {
        lender.require_auth();

        let mut offer: LoanOffer = env
            .storage()
            .persistent()
            .get(&DataKey::Offer(offer_id))
            .unwrap();

        if offer.lender != lender {
            panic!("Only lender can cancel");
        }
        if offer.status != OfferStatus::Open {
            panic!("Offer is not open");
        }

        offer.status = OfferStatus::Cancelled;
        env.storage().persistent().set(&DataKey::Offer(offer_id), &offer);

        Self::transfer_asset(
            &env,
            &offer.loan_asset,
            &env.current_contract_address(),
            &offer.lender,
        );
    }

    pub fn accept_offer(env: Env, borrower: Address, offer_id: u64) -> u64 {
        borrower.require_auth();

        let mut offer: LoanOffer = env
            .storage()
            .persistent()
            .get(&DataKey::Offer(offer_id))
            .unwrap();

        if offer.status != OfferStatus::Open {
            panic!("Offer is not open");
        }

        Self::transfer_asset(
            &env,
            &offer.collateral_asset,
            &borrower,
            &env.current_contract_address(),
        );

        let loan_id = Self::next_loan_id(&env);
        let start_time = env.ledger().timestamp();
        let due_time = start_time + offer.terms.duration_secs;

        let loan = Loan {
            loan_id,
            offer_id,
            lender: offer.lender.clone(),
            borrower: borrower.clone(),
            loan_asset: offer.loan_asset.clone(),
            collateral_asset: offer.collateral_asset.clone(),
            repayment_token: offer.repayment_token.clone(),
            principal: offer.principal,
            outstanding_principal: offer.principal,
            accrued_interest: 0,
            start_time,
            due_time,
            last_accrual_time: start_time,
            status: LoanStatus::Active,
            terms: offer.terms.clone(),
            extension_secs: 0,
        };

        offer.status = OfferStatus::Accepted;
        env.storage().persistent().set(&DataKey::Offer(offer_id), &offer);
        env.storage().persistent().set(&DataKey::Loan(loan_id), &loan);

        Self::transfer_asset(
            &env,
            &offer.loan_asset,
            &env.current_contract_address(),
            &borrower,
        );

        loan_id
    }

    pub fn repay(env: Env, borrower: Address, loan_id: u64, amount: i128) {
        borrower.require_auth();

        if amount <= 0 {
            panic!("Repayment amount must be greater than zero");
        }

        let mut loan: Loan = env
            .storage()
            .persistent()
            .get(&DataKey::Loan(loan_id))
            .unwrap();

        if loan.status != LoanStatus::Active {
            panic!("Loan is not active");
        }
        if loan.borrower != borrower {
            panic!("Only borrower can repay");
        }

        Self::accrue_interest(&env, &mut loan);

        let total_due = loan.outstanding_principal + loan.accrued_interest;
        if amount > total_due {
            panic!("Repayment exceeds amount due");
        }

        let token_client = token::Client::new(&env, &loan.repayment_token);
        token_client.transfer(&borrower, &loan.lender, &amount);

        let mut remaining = amount;
        if loan.accrued_interest > 0 {
            let pay_interest = if remaining < loan.accrued_interest {
                remaining
            } else {
                loan.accrued_interest
            };
            loan.accrued_interest -= pay_interest;
            remaining -= pay_interest;
        }
        if remaining > 0 {
            loan.outstanding_principal -= remaining;
        }

        if loan.outstanding_principal == 0 && loan.accrued_interest == 0 {
            loan.status = LoanStatus::Repaid;

            Self::transfer_asset(
                &env,
                &loan.collateral_asset,
                &env.current_contract_address(),
                &loan.borrower,
            );

            if loan.loan_asset.asset_type == AssetType::NFT {
                Self::transfer_asset(&env, &loan.loan_asset, &loan.borrower, &loan.lender);
            }
        }

        env.storage().persistent().set(&DataKey::Loan(loan_id), &loan);
    }

    pub fn liquidate(env: Env, lender: Address, loan_id: u64) {
        lender.require_auth();

        let mut loan: Loan = env
            .storage()
            .persistent()
            .get(&DataKey::Loan(loan_id))
            .unwrap();

        if loan.status != LoanStatus::Active {
            panic!("Loan is not active");
        }
        if loan.lender != lender {
            panic!("Only lender can liquidate");
        }
        if env.ledger().timestamp() <= loan.due_time {
            panic!("Loan is not in default");
        }

        Self::accrue_interest(&env, &mut loan);
        loan.status = LoanStatus::Liquidated;

        Self::transfer_asset(
            &env,
            &loan.collateral_asset,
            &env.current_contract_address(),
            &loan.lender,
        );

        env.storage().persistent().set(&DataKey::Loan(loan_id), &loan);
        env.storage()
            .persistent()
            .remove(&DataKey::ExtensionRequest(loan_id));
    }

    pub fn request_extension(env: Env, borrower: Address, loan_id: u64, extra_secs: u64) {
        borrower.require_auth();

        if extra_secs == 0 {
            panic!("Extension must be greater than zero");
        }

        let loan: Loan = env
            .storage()
            .persistent()
            .get(&DataKey::Loan(loan_id))
            .unwrap();

        if loan.status != LoanStatus::Active {
            panic!("Loan is not active");
        }
        if loan.borrower != borrower {
            panic!("Only borrower can request extension");
        }
        if env.ledger().timestamp() > loan.due_time {
            panic!("Loan is already due");
        }
        if loan.extension_secs + extra_secs > loan.terms.max_extension_secs {
            panic!("Extension exceeds maximum");
        }

        let request = ExtensionRequest {
            loan_id,
            borrower,
            requested_extension_secs: extra_secs,
            requested_time: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::ExtensionRequest(loan_id), &request);
    }

    pub fn approve_extension(env: Env, lender: Address, loan_id: u64) {
        lender.require_auth();

        let mut loan: Loan = env
            .storage()
            .persistent()
            .get(&DataKey::Loan(loan_id))
            .unwrap();

        if loan.status != LoanStatus::Active {
            panic!("Loan is not active");
        }
        if loan.lender != lender {
            panic!("Only lender can approve extension");
        }

        let request: ExtensionRequest = env
            .storage()
            .persistent()
            .get(&DataKey::ExtensionRequest(loan_id))
            .unwrap();

        if request.borrower != loan.borrower {
            panic!("Extension requester mismatch");
        }

        Self::accrue_interest(&env, &mut loan);
        loan.due_time += request.requested_extension_secs;
        loan.extension_secs += request.requested_extension_secs;

        env.storage().persistent().set(&DataKey::Loan(loan_id), &loan);
        env.storage()
            .persistent()
            .remove(&DataKey::ExtensionRequest(loan_id));
    }

    pub fn get_offer(env: Env, offer_id: u64) -> Option<LoanOffer> {
        env.storage().persistent().get(&DataKey::Offer(offer_id))
    }

    pub fn get_loan(env: Env, loan_id: u64) -> Option<Loan> {
        env.storage().persistent().get(&DataKey::Loan(loan_id))
    }

    pub fn get_extension_request(env: Env, loan_id: u64) -> Option<ExtensionRequest> {
        env.storage()
            .persistent()
            .get(&DataKey::ExtensionRequest(loan_id))
    }

    fn next_offer_id(env: &Env) -> u64 {
        let mut id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::OfferCount)
            .unwrap_or(0);
        id += 1;
        env.storage().persistent().set(&DataKey::OfferCount, &id);
        id
    }

    fn next_loan_id(env: &Env) -> u64 {
        let mut id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::LoanCount)
            .unwrap_or(0);
        id += 1;
        env.storage().persistent().set(&DataKey::LoanCount, &id);
        id
    }

    fn validate_asset(asset: &Asset) {
        match asset.asset_type {
            AssetType::Token => {
                if asset.amount <= 0 {
                    panic!("Token amount must be greater than zero");
                }
            }
            AssetType::NFT => {
                if asset.nft_id == 0 {
                    panic!("NFT id must be set");
                }
                if asset.amount != 1 {
                    panic!("NFT amount must be 1");
                }
            }
        }
    }

    fn transfer_asset(env: &Env, asset: &Asset, from: &Address, to: &Address) {
        match asset.asset_type {
            AssetType::Token => {
                let token_client = token::Client::new(env, &asset.contract);
                token_client.transfer(from, to, &asset.amount);
            }
            AssetType::NFT => {
                let transfer_args = (from.clone(), to.clone(), asset.nft_id);
                env.invoke_contract::<()>(
                    &asset.contract,
                    &Symbol::new(env, "transfer"),
                    transfer_args.into_val(env),
                );
            }
        }
    }

    fn accrue_interest(env: &Env, loan: &mut Loan) {
        let now = env.ledger().timestamp();
        let accrual_end = if now < loan.due_time {
            now
        } else {
            loan.due_time
        };
        if accrual_end <= loan.last_accrual_time {
            return;
        }
        let duration = loan.due_time - loan.start_time;
        if duration == 0 || loan.outstanding_principal == 0 {
            loan.last_accrual_time = accrual_end;
            return;
        }
        let delta = accrual_end - loan.last_accrual_time;
        let interest = (loan.outstanding_principal
            * loan.terms.interest_bps as i128
            * delta as i128)
            / duration as i128
            / BASIS_POINTS;
        loan.accrued_interest += interest;
        loan.last_accrual_time = accrual_end;
    }
}

#[cfg(test)]
mod test;

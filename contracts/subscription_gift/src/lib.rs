#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    Address, Bytes, BytesN, Env, Vec,
};

#[contract]
pub struct GiftContract;

#[derive(Clone, PartialEq)]
#[contracttype]
pub enum GiftStatus {
    Pending,
    Claimed,
    Refunded,
}

#[derive(Clone)]
#[contracttype]
pub struct GiftCode {
    pub code_hash: BytesN<32>,
    pub sender: Address,
    pub recipient: Address,
    pub product_id: u32,
    pub expires_at: u64,
    pub status: GiftStatus,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Gift(BytesN<32>),
    SentList(Address),
}

const DAY: u64 = 86400;

#[contractimpl]
impl GiftContract {

    pub fn create_gift(
        env: Env,
        sender: Address,
        recipient: Address,
        product_id: u32,
        duration_days: u64,
    ) -> Bytes {

        sender.require_auth();

        let raw_code = Bytes::from_array(&env, &[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16]);
        let code_hash: BytesN<32> = env.crypto().sha256(&raw_code).into();

        let expires_at = env.ledger().timestamp() + duration_days * DAY;

        let gift = GiftCode {
            code_hash: code_hash.clone(),
            sender: sender.clone(),
            recipient: recipient.clone(),
            product_id,
            expires_at,
            status: GiftStatus::Pending,
        };

        env.storage().persistent().set(&DataKey::Gift(code_hash.clone()), &gift);

        let mut list: Vec<BytesN<32>> =
            env.storage()
                .persistent()
                .get(&DataKey::SentList(sender.clone()))
                .unwrap_or(Vec::new(&env));

        list.push_back(code_hash.clone());
        env.storage().persistent().set(&DataKey::SentList(sender.clone()), &list);

        raw_code
    }

    pub fn claim_gift(env: Env, caller: Address, raw_code: Bytes) {

        caller.require_auth();

        let code_hash: BytesN<32> = env.crypto().sha256(&raw_code).into();

        let mut gift: GiftCode = match env.storage()
            .persistent()
            .get(&DataKey::Gift(code_hash.clone())) {
                Some(g) => g,
                None => return,
        };

        if gift.status != GiftStatus::Pending {
            return;
        }

        if env.ledger().timestamp() > gift.expires_at {
            return;
        }

        if caller != gift.recipient {
            return;
        }

        gift.status = GiftStatus::Claimed;

        env.storage().persistent().set(&DataKey::Gift(code_hash.clone()), &gift);
    }

    pub fn refund_gift(env: Env, caller: Address, code_hash: BytesN<32>) {

        caller.require_auth();

        let mut gift: GiftCode = match env.storage()
            .persistent()
            .get(&DataKey::Gift(code_hash.clone())) {
                Some(g) => g,
                None => return,
        };

        if caller != gift.sender {
            return;
        }

        if gift.status != GiftStatus::Pending {
            return;
        }

        if env.ledger().timestamp() < gift.expires_at {
            return;
        }

        gift.status = GiftStatus::Refunded;

        env.storage().persistent().set(&DataKey::Gift(code_hash.clone()), &gift);
    }

    pub fn get_gift(env: Env, code_hash: BytesN<32>) -> GiftCode {
        env.storage()
            .persistent()
            .get(&DataKey::Gift(code_hash))
            .unwrap()
    }

    pub fn list_sent_gifts(env: Env, sender: Address) -> Vec<BytesN<32>> {
        env.storage()
            .persistent()
            .get(&DataKey::SentList(sender))
            .unwrap_or(Vec::new(&env))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{Env, testutils::Address as _};

    #[test]
    fn test_create_and_claim() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, GiftContract);
        let client = GiftContractClient::new(&env, &contract_id);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);

        let code = client.create_gift(&sender, &recipient, &1, &30);
        client.claim_gift(&recipient, &code);
    }

    #[test]
    fn test_double_claim_safe() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, GiftContract);
        let client = GiftContractClient::new(&env, &contract_id);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);

        let code = client.create_gift(&sender, &recipient, &1, &30);

        client.claim_gift(&recipient, &code);
        client.claim_gift(&recipient, &code); // no crash now
    }
}
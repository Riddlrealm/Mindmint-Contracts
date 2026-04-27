#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env, Vec, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PaymentSplitterError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    SplitNotFound = 4,
    InvalidShares = 5,
    ConfigLocked = 6,
    InsufficientBalance = 7,
    NothingToRelease = 8,
}

#[contracttype]
#[derive(Clone)]
pub struct Recipient {
    pub address: Address,
    pub shares: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct SplitConfig {
    pub id: u64,
    pub recipients: Vec<Recipient>,
    pub token: Address,
    pub total_released: i128,
    pub locked: bool,
}

#[contracttype]
pub enum DataKey {
    Admin,
    SplitId,
    SplitConfig(u64),
}

#[contract]
pub struct PaymentSplitter;

#[contractimpl]
impl PaymentSplitter {
    /// Initialize the payment splitter contract
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::SplitId, &0u64);
    }

    /// Create a new payment split configuration
    pub fn create_split(env: Env, admin: Address, token: Address, recipients: Vec<Recipient>) -> u64 {
        admin.require_auth();
        Self::require_admin(&env);

        // Validate shares sum
        let mut total_shares: u64 = 0;
        for recipient in recipients.iter() {
            total_shares += recipient.shares;
        }

        if total_shares == 0 {
            panic!("Invalid shares: total cannot be zero");
        }

        // Get next split ID
        let split_id = env.storage().instance().get::<_, u64>(&DataKey::SplitId).unwrap();
        let next_split_id = split_id + 1;

        // Create split config
        let config = SplitConfig {
            id: split_id,
            recipients,
            token,
            total_released: 0,
            locked: false,
        };

        env.storage().persistent().set(&DataKey::SplitConfig(split_id), &config);
        env.storage().instance().set(&DataKey::SplitId, &next_split_id);

        split_id
    }

    /// Deposit tokens to a split
    pub fn deposit(env: Env, split_id: u64, from: Address, amount: i128) {
        from.require_auth();

        let mut config = env.storage().persistent().get::<_, SplitConfig>(&DataKey::SplitConfig(split_id))
            .expect("Split not found");

        if amount <= 0 {
            panic!("Invalid amount");
        }

        // Transfer tokens to contract
        let token_client = token::Client::new(&env, &config.token);
        token_client.transfer(&from, &env.current_contract_address(), &amount);

        // Lock config after first deposit
        config.locked = true;
        env.storage().persistent().set(&DataKey::SplitConfig(split_id), &config);

        env.events().publish(
            (Symbol::short("deposit"), split_id),
            amount,
        );
    }

    /// Release tokens for all recipients in a split
    pub fn release(env: Env, split_id: u64) {
        let config = env.storage().persistent().get::<_, SplitConfig>(&DataKey::SplitConfig(split_id))
            .expect("Split not found");

        // Calculate total shares
        let mut total_shares: u64 = 0;
        for recipient in config.recipients.iter() {
            total_shares += recipient.shares;
        }

        // Get contract token balance
        let token_client = token::Client::new(&env, &config.token);
        let contract_balance = token_client.balance(&env.current_contract_address());
        let unreleased = contract_balance - config.total_released;

        if unreleased <= 0 {
            panic!("Nothing to release");
        }

        // Distribute to each recipient
        for recipient in config.recipients.iter() {
            let share = (unreleased * recipient.shares as i128) / total_shares as i128;
            if share > 0 {
                token_client.transfer(&env.current_contract_address(), &recipient.address, &share);

                env.events().publish(
                    (Symbol::short("release"), split_id, recipient.address.clone()),
                    share,
                );
            }
        }

        // Update total released
        let mut config = env.storage().persistent().get::<_, SplitConfig>(&DataKey::SplitConfig(split_id))
            .expect("Split not found");
        config.total_released = contract_balance;
        env.storage().persistent().set(&DataKey::SplitConfig(split_id), &config);
    }

    /// Release tokens for a specific recipient
    pub fn release_to(env: Env, split_id: u64, recipient_address: Address) {
        let config = env.storage().persistent().get::<_, SplitConfig>(&DataKey::SplitConfig(split_id))
            .expect("Split not found");

        // Calculate total shares
        let mut total_shares: u64 = 0;
        let mut recipient_shares: u64 = 0;
        for recipient in config.recipients.iter() {
            total_shares += recipient.shares;
            if recipient.address == recipient_address {
                recipient_shares = recipient.shares;
            }
        }

        if recipient_shares == 0 {
            panic!("Recipient not found");
        }

        // Get contract token balance
        let token_client = token::Client::new(&env, &config.token);
        let contract_balance = token_client.balance(&env.current_contract_address());
        let unreleased = contract_balance - config.total_released;

        if unreleased <= 0 {
            panic!("Nothing to release");
        }

        // Calculate share for this recipient
        let share = (unreleased * recipient_shares as i128) / total_shares as i128;

        if share > 0 {
            token_client.transfer(&env.current_contract_address(), &recipient_address, &share);

            env.events().publish(
                (Symbol::short("release"), split_id, recipient_address),
                share,
            );
        }

        // Update total released (note: this is a simplified approach)
        let mut config = env.storage().persistent().get::<_, SplitConfig>(&DataKey::SplitConfig(split_id))
            .expect("Split not found");
        config.total_released += share;
        env.storage().persistent().set(&DataKey::SplitConfig(split_id), &config);
    }

    /// Get split configuration
    pub fn get_split(env: Env, split_id: u64) -> SplitConfig {
        env.storage().persistent().get::<_, SplitConfig>(&DataKey::SplitConfig(split_id))
            .expect("Split not found")
    }

    /// Update split recipients (admin only, only if not locked)
    pub fn update_recipients(env: Env, admin: Address, split_id: u64, recipients: Vec<Recipient>) {
        admin.require_auth();
        Self::require_admin(&env);

        let mut config = env.storage().persistent().get::<_, SplitConfig>(&DataKey::SplitConfig(split_id))
            .expect("Split not found");

        if config.locked {
            panic!("Config locked");
        }

        // Validate shares sum
        let mut total_shares: u64 = 0;
        for recipient in recipients.iter() {
            total_shares += recipient.shares;
        }

        if total_shares == 0 {
            panic!("Invalid shares: total cannot be zero");
        }

        config.recipients = recipients;
        env.storage().persistent().set(&DataKey::SplitConfig(split_id), &config);
    }

    fn require_admin(env: &Env) {
        let admin = env.storage().instance().get::<_, Address>(&DataKey::Admin).unwrap();
        admin.require_auth();
    }

    fn require_init(env: &Env) {
        if !env.storage().instance().has(&DataKey::Admin) {
            panic!("Not initialized");
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::token::StellarAssetClient;

    fn create_token_contract(env: &Env, admin: &Address) -> Address {
        env.register_stellar_asset_contract_v2(admin.clone()).address()
    }

    fn setup() -> (Env, Address, Address, StellarAssetClient<'static>, PaymentSplitterClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let token = create_token_contract(&env, &token_admin);

        let token_admin_client = StellarAssetClient::new(&env, &token);

        let contract_id = env.register_contract(None, PaymentSplitter);
        let client = PaymentSplitterClient::new(&env, &contract_id);

        client.initialize(&admin);

        (env, admin.clone(), token, token_admin_client, client, admin.clone())
    }

    #[test]
    fn test_initialize() {
        let (env, admin, _token, _token_admin_client, _client, _admin) = setup();
        let contract_id = env.register_contract(None, PaymentSplitter);
        let client = PaymentSplitterClient::new(&env, &contract_id);

        client.initialize(&admin);
    }

    #[test]
    fn test_create_split() {
        let (env, _admin, token, _token_admin_client, client, admin) = setup();

        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);

        let recipients = Vec::from_array(&env, [
            Recipient { address: recipient1.clone(), shares: 50 },
            Recipient { address: recipient2.clone(), shares: 50 },
        ]);

        let split_id = client.create_split(&admin, &token, &recipients);

        assert_eq!(split_id, 0);

        let config = client.get_split(&split_id);
        assert_eq!(config.id, 0);
        assert_eq!(config.recipients.len(), 2);
        assert!(!config.locked);
    }

    #[test]
    fn test_deposit() {
        let (env, _admin, token, token_admin_client, client, admin) = setup();

        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);

        let recipients = Vec::from_array(&env, [
            Recipient { address: recipient1.clone(), shares: 50 },
            Recipient { address: recipient2.clone(), shares: 50 },
        ]);

        let split_id = client.create_split(&admin, &token, &recipients);

        // Mint tokens to admin
        token_admin_client.mint(&admin, &1_000_000);

        client.deposit(&split_id, &admin, &1_000_000);

        let config = client.get_split(&split_id);
        assert!(config.locked);
    }

    #[test]
    fn test_release() {
        let (env, _admin, token, token_admin_client, client, admin) = setup();

        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);

        let recipients = Vec::from_array(&env, [
            Recipient { address: recipient1.clone(), shares: 50 },
            Recipient { address: recipient2.clone(), shares: 50 },
        ]);

        let split_id = client.create_split(&admin, &token, &recipients);

        // Mint and deposit tokens
        token_admin_client.mint(&admin, &1_000_000);
        client.deposit(&split_id, &admin, &1_000_000);

        // Release to all recipients
        client.release(&split_id);

        let config = client.get_split(&split_id);
        assert_eq!(config.total_released, 1_000_000);
    }

    #[test]
    fn test_release_to() {
        let (env, _admin, token, token_admin_client, client, admin) = setup();

        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);

        let recipients = Vec::from_array(&env, [
            Recipient { address: recipient1.clone(), shares: 60 },
            Recipient { address: recipient2.clone(), shares: 40 },
        ]);

        let split_id = client.create_split(&admin, &token, &recipients);

        // Mint and deposit tokens
        token_admin_client.mint(&admin, &1_000_000);
        client.deposit(&split_id, &admin, &1_000_000);

        // Release to specific recipient
        client.release_to(&split_id, &recipient1);

        let config = client.get_split(&split_id);
        assert!(config.total_released > 0);
    }

    #[test]
    #[should_panic(expected = "Invalid shares")]
    fn test_invalid_shares() {
        let (env, _admin, token, _token_admin_client, client, admin) = setup();

        let recipient = Address::generate(&env);
        let recipients = Vec::from_array(&env, [
            Recipient { address: recipient, shares: 0 },
        ]);

        client.create_split(&admin, &token, &recipients);
    }

    #[test]
    fn test_update_recipients() {
        let (env, _admin, token, _token_admin_client, client, admin) = setup();

        let recipient1 = Address::generate(&env);
        let recipient2 = Address::generate(&env);

        let recipients = Vec::from_array(&env, [
            Recipient { address: recipient1.clone(), shares: 100 },
        ]);

        let split_id = client.create_split(&admin, &token, &recipients);

        let recipient3 = Address::generate(&env);
        let updated_recipients = Vec::from_array(&env, [
            Recipient { address: recipient1.clone(), shares: 50 },
            Recipient { address: recipient3.clone(), shares: 50 },
        ]);

        client.update_recipients(&admin, &split_id, &updated_recipients);

        let config = client.get_split(&split_id);
        assert_eq!(config.recipients.len(), 2);
    }

    #[test]
    #[should_panic(expected = "Config locked")]
    fn test_config_locked() {
        let (env, _admin, token, token_admin_client, client, admin) = setup();

        let recipient1 = Address::generate(&env);
        let recipients = Vec::from_array(&env, [
            Recipient { address: recipient1.clone(), shares: 100 },
        ]);

        let split_id = client.create_split(&admin, &token, &recipients);

        // Deposit to lock config
        token_admin_client.mint(&admin, &1_000_000);
        client.deposit(&split_id, &admin, &1_000_000);

        // Try to update recipients (should fail)
        let recipient2 = Address::generate(&env);
        let updated_recipients = Vec::from_array(&env, [
            Recipient { address: recipient2, shares: 100 },
        ]);

        client.update_recipients(&admin, &split_id, &updated_recipients);
    }
}

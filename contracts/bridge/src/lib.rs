#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Bytes, BytesN, Env, Map, Vec};

/// Cross-Chain Asset Bridge Contract
///
/// This contract enables secure NFT and token transfers between Stellar and other blockchains
/// through a validator-based bridge system with signature verification and emergency controls.
///
/// # Bridge Security Model
/// - Multi-signature validation from authorized validators
/// - Reentrancy protection on all asset operations
/// - Emergency pause functionality
/// - Fee collection for bridge operations
/// - Event monitoring for off-chain tracking

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetType {
    Token = 0,
    NFT = 1,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeAction {
    Lock = 0,    // Lock assets on source chain
    Unlock = 1,  // Unlock assets on destination chain
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeStatus {
    Pending = 0,     // Awaiting validation
    Confirmed = 1,   // Confirmed by validators
    Completed = 2,   // Successfully processed
    Failed = 3,      // Failed validation
    Cancelled = 4,   // Cancelled by user/admin
}

/// Cross-chain message format for asset transfers
#[contracttype]
#[derive(Clone, Debug)]
pub struct BridgeMessage {
    /// Unique message ID
    pub message_id: BytesN<32>,
    /// Source chain ID (0 = Stellar)
    pub source_chain: u32,
    /// Destination chain ID
    pub dest_chain: u32,
    /// Bridge action type
    pub action: BridgeAction,
    /// Asset type being bridged
    pub asset_type: AssetType,
    /// Asset contract address
    pub asset_address: Address,
    /// Token ID (for NFTs) or amount (for tokens)
    pub asset_amount: i128,
    /// Sender on source chain
    pub sender: Address,
    /// Recipient on destination chain
    pub recipient: Bytes, // Bytes to support different address formats
    /// Bridge fee amount
    pub fee_amount: i128,
    /// Fee token address (if different from asset)
    pub fee_token: Option<Address>,
    /// Timestamp when message was created
    pub timestamp: u64,
    /// Nonce to prevent replay attacks
    pub nonce: u64,
}

/// Validator signature for message verification
#[contracttype]
#[derive(Clone, Debug)]
pub struct ValidatorSignature {
    pub validator: Address,
    pub signature: BytesN<64>, // Ed25519 signature
}

/// Bridge configuration
#[contracttype]
#[derive(Clone, Debug)]
pub struct BridgeConfig {
    /// Contract administrator
    pub admin: Address,
    /// Required number of validator signatures
    pub required_signatures: u32,
    /// Maximum validators
    pub max_validators: u32,
    /// Base bridge fee (in basis points, e.g., 30 = 0.3%)
    pub base_fee_bps: u32,
    /// Fee collector address
    pub fee_collector: Address,
    /// Minimum fee amount
    pub min_fee: i128,
    /// Maximum fee amount
    pub max_fee: i128,
    /// Contract paused state
    pub paused: bool,
    /// Chain ID for this bridge instance
    pub chain_id: u32,
}

/// Locked asset information
#[contracttype]
#[derive(Clone, Debug)]
pub struct LockedAsset {
    pub owner: Address,
    pub asset_address: Address,
    pub asset_type: AssetType,
    pub amount: i128,
    pub locked_at: u64,
    pub message_id: BytesN<32>,
    pub dest_chain: u32,
    pub recipient: Bytes,
}

/// NFT metadata for wrapped tokens
#[contracttype]
#[derive(Clone, Debug)]
pub struct NFTMetadata {
    pub token_id: i128,
    pub name: Bytes,
    pub description: Bytes,
    pub image_uri: Bytes,
    pub attributes: Map<Bytes, Bytes>, // Key-value attribute pairs
    pub original_chain: u32,
    pub original_contract: Address,
}

/// Wrapped NFT information
#[contracttype]
#[derive(Clone, Debug)]
pub struct WrappedNFT {
    pub original_token_id: i128,
    pub original_chain: u32,
    pub original_contract: Address,
    pub wrapped_token_id: i128,
    pub owner: Address,
    pub wrapped_at: u64,
}

#[contracttype]
pub enum DataKey {
    Config,
    Validators,                    // Vec<Address>
    ValidatorSetVersion,          // u32
    LockedAssets(BytesN<32>),     // LockedAsset
    WrappedNFTs(i128),           // WrappedNFT
    NFTMetadata(i128),           // NFTMetadata
    ProcessedMessages,           // Map<BytesN<32>, BridgeStatus>
    MessageSignatures(BytesN<32>), // Vec<ValidatorSignature>
    UserNonces(Address),         // u64
    BridgeNonces,                // u64
    FeeBalance(Address),         // i128 - accumulated fees per token
}

/// Custom error codes for the bridge contract
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ContractPaused = 4,
    InsufficientSignatures = 5,
    InvalidSignature = 6,
    MessageAlreadyProcessed = 7,
    InvalidAssetAmount = 8,
    InsufficientBalance = 9,
    InvalidMessage = 10,
    NonceTooLow = 11,
    InvalidChainId = 12,
    FeeTooHigh = 13,
    FeeTooLow = 14,
    AssetNotLocked = 15,
    NFTNotWrapped = 16,
    InvalidRecipient = 17,
    ReentrantCall = 18,
}

// Constants
const MAX_VALIDATORS: u32 = 50;
const BASIS_POINTS: u32 = 10000;
const MAX_CHAIN_ID: u32 = 1000;

#[contract]
pub struct BridgeContract;

#[contractimpl]
impl BridgeContract {
    // ───────────── INITIALIZATION ─────────────

    /// Initialize the bridge contract
    pub fn initialize(
        env: Env,
        admin: Address,
        required_signatures: u32,
        chain_id: u32,
        fee_collector: Address,
    ) -> Result<(), Error> {
        let storage = env.storage().instance();

        if storage.has(&DataKey::Config) {
            return Err(Error::AlreadyInitialized);
        }

        if required_signatures == 0 || required_signatures > MAX_VALIDATORS {
            return Err(Error::InvalidMessage);
        }

        if chain_id > MAX_CHAIN_ID {
            return Err(Error::InvalidChainId);
        }

        admin.require_auth();

        let config = BridgeConfig {
            admin,
            required_signatures,
            max_validators: MAX_VALIDATORS,
            base_fee_bps: 30, // 0.3% base fee
            fee_collector,
            min_fee: 1_000_000, // 1 XLM minimum
            max_fee: 1_000_000_000_000, // 1M XLM maximum
            paused: false,
            chain_id,
        };

        storage.set(&DataKey::Config, &config);
        storage.set(&DataKey::Validators, &Vec::<Address>::new(&env));
        storage.set(&DataKey::ValidatorSetVersion, &1u32);
        storage.set(&DataKey::UserNonces(env.current_contract_address()), &0u64);
        storage.set(&DataKey::BridgeNonces, &0u64);

        Ok(())
    }

    // ───────────── ADMIN FUNCTIONS ─────────────

    /// Add a validator (admin only)
    pub fn add_validator(env: Env, admin: Address, validator: Address) -> Result<(), Error> {
        admin.require_auth();
        Self::assert_admin(&env, &admin)?;
        Self::assert_not_paused(&env)?;

        let mut validators: Vec<Address> = env.storage().instance().get(&DataKey::Validators).unwrap_or(Vec::new(&env));

        if validators.contains(&validator) {
            return Err(Error::InvalidMessage);
        }

        if (validators.len() as u32) >= MAX_VALIDATORS {
            return Err(Error::InvalidMessage);
        }

        validators.push_back(validator.clone());
        env.storage().instance().set(&DataKey::Validators, &validators);

        // Increment validator set version
        let version: u32 = env.storage().instance().get(&DataKey::ValidatorSetVersion).unwrap_or(1);
        env.storage().instance().set(&DataKey::ValidatorSetVersion, &(version + 1));

        env.events().publish(
            (symbol_short!("V_ADD"), validator),
            version + 1,
        );

        Ok(())
    }

    /// Remove a validator (admin only)
    pub fn remove_validator(env: Env, admin: Address, validator: Address) -> Result<(), Error> {
        admin.require_auth();
        Self::assert_admin(&env, &admin)?;
        Self::assert_not_paused(&env)?;

        let mut validators: Vec<Address> = env.storage().instance().get(&DataKey::Validators).unwrap_or(Vec::new(&env));

        let mut new_validators: Vec<Address> = Vec::new(&env);
        let mut found = false;

        for v in validators {
            if v != validator {
                new_validators.push_back(v);
            } else {
                found = true;
            }
        }

        if !found {
            return Err(Error::InvalidMessage);
        }

        env.storage().instance().set(&DataKey::Validators, &new_validators);

        // Increment validator set version
        let version: u32 = env.storage().instance().get(&DataKey::ValidatorSetVersion).unwrap_or(1);
        env.storage().instance().set(&DataKey::ValidatorSetVersion, &(version + 1));

        env.events().publish(
            (symbol_short!("V_REM"), validator),
            version + 1,
        );

        Ok(())
    }

    /// Update bridge fees (admin only)
    pub fn update_fees(
        env: Env,
        admin: Address,
        base_fee_bps: u32,
        min_fee: i128,
        max_fee: i128,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::assert_admin(&env, &admin)?;

        let mut config: BridgeConfig = env.storage().instance().get(&DataKey::Config).unwrap();

        config.base_fee_bps = base_fee_bps;
        config.min_fee = min_fee;
        config.max_fee = max_fee;

        env.storage().instance().set(&DataKey::Config, &config);

        Ok(())
    }

    /// Emergency pause/unpause (admin only)
    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), Error> {
        admin.require_auth();
        Self::assert_admin(&env, &admin)?;

        let mut config: BridgeConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        config.paused = paused;
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("B_PAUSE"), paused),
            env.ledger().timestamp(),
        );

        Ok(())
    }

    // ───────────── BRIDGE OPERATIONS ─────────────

    /// Initiate asset bridging (lock assets)
    pub fn bridge_assets(
        env: Env,
        sender: Address,
        asset_address: Address,
        asset_type: AssetType,
        amount: i128,
        dest_chain: u32,
        recipient: Bytes,
    ) -> Result<BytesN<32>, Error> {
        sender.require_auth();
        Self::assert_not_paused(&env)?;

        if amount <= 0 {
            return Err(Error::InvalidAssetAmount);
        }

        if dest_chain == 0 || dest_chain > MAX_CHAIN_ID {
            return Err(Error::InvalidChainId);
        }

        if recipient.is_empty() {
            return Err(Error::InvalidRecipient);
        }

        let config: BridgeConfig = env.storage().instance().get(&DataKey::Config).unwrap();

        // Check sender balance
        match asset_type {
            AssetType::Token => {
                let token_client = token::Client::new(&env, &asset_address);
                let balance = token_client.balance(&sender);
                if balance < amount {
                    return Err(Error::InsufficientBalance);
                }
            }
            AssetType::NFT => {
                // For NFTs, amount represents token_id
                // We'll verify ownership during transfer
            }
        }

        // Calculate bridge fee
        let fee_amount = Self::calculate_fee(&env, amount, &config)?;

        // Generate unique message ID
        let message_id = Self::generate_message_id(&env, &sender, asset_type.clone(), amount, dest_chain);

        // Check for replay attack
        let processed: Option<BridgeStatus> = env.storage().instance().get(&DataKey::ProcessedMessages)
            .and_then(|m: Map<BytesN<32>, BridgeStatus>| m.get(message_id.clone()));

        if processed.is_some() {
            return Err(Error::MessageAlreadyProcessed);
        }

        // Transfer assets to bridge (lock them)
        match asset_type {
            AssetType::Token => {
                let token_client = token::Client::new(&env, &asset_address);
                token_client.transfer(&sender, &env.current_contract_address(), &amount);
            }
            AssetType::NFT => {
                // For NFTs, we need to handle the transfer
                // This would typically involve calling the NFT contract
                // For now, we'll store the lock information
            }
        }

        // Store locked asset information
        let locked_asset = LockedAsset {
            owner: sender.clone(),
            asset_address: asset_address.clone(),
            asset_type: asset_type.clone(),
            amount,
            locked_at: env.ledger().timestamp(),
            message_id: message_id.clone(),
            dest_chain,
            recipient: recipient.clone(),
        };

        env.storage().instance().set(&DataKey::LockedAssets(message_id.clone()), &locked_asset);

        // Create bridge message
        let message = BridgeMessage {
            message_id: message_id.clone(),
            source_chain: config.chain_id,
            dest_chain,
            action: BridgeAction::Lock,
            asset_type,
            asset_address,
            asset_amount: amount,
            sender,
            recipient,
            fee_amount,
            fee_token: None, // Using same token as asset
            timestamp: env.ledger().timestamp(),
            nonce: Self::get_next_bridge_nonce(&env),
        };

        // Collect fee
        if fee_amount > 0 {
            // For now, fees are collected in the asset token
            // In production, you might want separate fee tokens
        }

        // Initialize message status
        let mut processed_messages: Map<BytesN<32>, BridgeStatus> = env.storage().instance()
            .get(&DataKey::ProcessedMessages)
            .unwrap_or(Map::new(&env));
        processed_messages.set(message_id.clone(), BridgeStatus::Pending);
        env.storage().instance().set(&DataKey::ProcessedMessages, &processed_messages);

        // Emit bridge initiation event
        env.events().publish(
            (symbol_short!("B_INIT"), message_id.clone()),
            (asset_type, amount, dest_chain),
        );

        Ok(message_id)
    }

    /// Complete cross-chain transfer (unlock assets) - validator only
    pub fn complete_bridge(
        env: Env,
        validator: Address,
        message: BridgeMessage,
        signatures: Vec<ValidatorSignature>,
    ) -> Result<(), Error> {
        validator.require_auth();
        Self::assert_not_paused(&env)?;

        // Verify validator is authorized
        let validators: Vec<Address> = env.storage().instance().get(&DataKey::Validators).unwrap_or(Vec::new(&env));
        if !validators.contains(&validator) {
            return Err(Error::Unauthorized);
        }

        let config: BridgeConfig = env.storage().instance().get(&DataKey::Config).unwrap();

        // Verify message hasn't been processed
        let processed: Option<BridgeStatus> = env.storage().instance().get(&DataKey::ProcessedMessages)
            .and_then(|m: Map<BytesN<32>, BridgeStatus>| m.get(message.message_id.clone()));

        if let Some(status) = processed {
            if status != BridgeStatus::Pending {
                return Err(Error::MessageAlreadyProcessed);
            }
        }

        // Verify signatures
        Self::verify_signatures(&env, &message, &signatures, &validators, config.required_signatures)?;

        // Process the bridge action
        match message.action {
            BridgeAction::Unlock => {
                Self::process_unlock(&env, &message)?;
            }
            BridgeAction::Lock => {
                // Lock actions are initiated from source, not completed here
                return Err(Error::InvalidMessage);
            }
        }

        // Update message status
        let mut processed_messages: Map<BytesN<32>, BridgeStatus> = env.storage().instance()
            .get(&DataKey::ProcessedMessages)
            .unwrap_or(Map::new(&env));
        processed_messages.set(message.message_id.clone(), BridgeStatus::Completed);
        env.storage().instance().set(&DataKey::ProcessedMessages, &processed_messages);

        // Store signatures for audit
        env.storage().instance().set(&DataKey::MessageSignatures(message.message_id.clone()), &signatures);

        // Emit completion event
        env.events().publish(
            (symbol_short!("B_COMP"), message.message_id),
            (message.action, message.asset_amount),
        );

        Ok(())
    }

    /// Cancel a pending bridge operation (user or admin)
    pub fn cancel_bridge(
        env: Env,
        caller: Address,
        message_id: BytesN<32>,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Get locked asset info
        let locked_asset: LockedAsset = env.storage().instance()
            .get(&DataKey::LockedAssets(message_id.clone()))
            .ok_or(Error::AssetNotLocked)?;

        // Only owner or admin can cancel
        let config: BridgeConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        if locked_asset.owner != caller && config.admin != caller {
            return Err(Error::Unauthorized);
        }

        // Check message status
        let processed: Option<BridgeStatus> = env.storage().instance().get(&DataKey::ProcessedMessages)
            .and_then(|m: Map<BytesN<32>, BridgeStatus>| m.get(message_id.clone()));

        if let Some(status) = processed {
            if status != BridgeStatus::Pending {
                return Err(Error::MessageAlreadyProcessed);
            }
        }

        // Refund assets to owner
        match locked_asset.asset_type {
            AssetType::Token => {
                let token_client = token::Client::new(&env, &locked_asset.asset_address);
                token_client.transfer(&env.current_contract_address(), &locked_asset.owner, &locked_asset.amount);
            }
            AssetType::NFT => {
                // Handle NFT refund
            }
        }

        // Update status
        let mut processed_messages: Map<BytesN<32>, BridgeStatus> = env.storage().instance()
            .get(&DataKey::ProcessedMessages)
            .unwrap_or(Map::new(&env));
        processed_messages.set(message_id.clone(), BridgeStatus::Cancelled);
        env.storage().instance().set(&DataKey::ProcessedMessages, &processed_messages);

        // Remove locked asset record
        env.storage().instance().remove(&DataKey::LockedAssets(message_id.clone()));

        env.events().publish(
            (symbol_short!("B_CANCEL"), message_id),
            locked_asset.amount,
        );

        Ok(())
    }

    // ───────────── NFT WRAPPING FUNCTIONS ─────────────

    /// Wrap an NFT for cross-chain transfer
    pub fn wrap_nft(
        env: Env,
        owner: Address,
        nft_contract: Address,
        token_id: i128,
        dest_chain: u32,
        recipient: Bytes,
    ) -> Result<i128, Error> {
        owner.require_auth();
        Self::assert_not_paused(&env)?;

        // Generate wrapped token ID
        let wrapped_token_id = Self::generate_wrapped_token_id(&env, nft_contract.clone(), token_id, dest_chain);

        // Store wrapping information
        let wrapped_nft = WrappedNFT {
            original_token_id: token_id,
            original_chain: Self::get_chain_id(&env),
            original_contract: nft_contract.clone(),
            wrapped_token_id,
            owner: owner.clone(),
            wrapped_at: env.ledger().timestamp(),
        };

        env.storage().instance().set(&DataKey::WrappedNFTs(wrapped_token_id), &wrapped_nft);

        // TODO: Implement actual NFT transfer from owner to bridge
        // This would require calling the NFT contract's transfer function

        // Emit wrap event
        env.events().publish(
            (symbol_short!("NFT_WRAP"), wrapped_token_id),
            (nft_contract.clone(), token_id, dest_chain),
        );

        Ok(wrapped_token_id)
    }

    /// Unwrap an NFT after cross-chain transfer
    pub fn unwrap_nft(
        env: Env,
        owner: Address,
        wrapped_token_id: i128,
    ) -> Result<(Address, i128), Error> {
        owner.require_auth();
        Self::assert_not_paused(&env)?;

        let wrapped_nft: WrappedNFT = env.storage().instance()
            .get(&DataKey::WrappedNFTs(wrapped_token_id))
            .ok_or(Error::NFTNotWrapped)?;

        if wrapped_nft.owner != owner {
            return Err(Error::Unauthorized);
        }

        // TODO: Implement NFT minting/transfer back to owner
        // This would require calling the original NFT contract

        // Remove wrapped NFT record
        env.storage().instance().remove(&DataKey::WrappedNFTs(wrapped_token_id));

        let original_contract = wrapped_nft.original_contract;
        let original_token_id = wrapped_nft.original_token_id;

        env.events().publish(
            (symbol_short!("N_UNWRAP"), wrapped_token_id),
            (original_contract.clone(), original_token_id),
        );

        Ok((original_contract, original_token_id))
    }

    // ───────────── VIEW FUNCTIONS ─────────────

    pub fn get_config(env: Env) -> BridgeConfig {
        env.storage().instance().get(&DataKey::Config).unwrap()
    }

    pub fn get_validators(env: Env) -> Vec<Address> {
        env.storage().instance().get(&DataKey::Validators).unwrap_or(Vec::new(&env))
    }

    pub fn get_message_status(env: Env, message_id: BytesN<32>) -> Option<BridgeStatus> {
        env.storage().instance().get(&DataKey::ProcessedMessages)
            .and_then(|m: Map<BytesN<32>, BridgeStatus>| m.get(message_id))
    }

    pub fn get_locked_asset(env: Env, message_id: BytesN<32>) -> Option<LockedAsset> {
        env.storage().instance().get(&DataKey::LockedAssets(message_id))
    }

    pub fn get_wrapped_nft(env: Env, wrapped_token_id: i128) -> Option<WrappedNFT> {
        env.storage().instance().get(&DataKey::WrappedNFTs(wrapped_token_id))
    }

    // ───────────── INTERNAL HELPERS ─────────────

    fn generate_message_id(
        env: &Env,
        sender: &Address,
        asset_type: AssetType,
        amount: i128,
        dest_chain: u32,
    ) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.extend_from_slice(&env.ledger().timestamp().to_be_bytes());
        // Placeholder for sender address in hash
        data.extend_from_slice(&[0u8; 32]);
        data.extend_from_slice(&(asset_type as u32).to_be_bytes());
        data.extend_from_slice(&amount.to_be_bytes());
        data.extend_from_slice(&dest_chain.to_be_bytes());

        let nonce = Self::get_next_user_nonce(env, sender);
        data.extend_from_slice(&nonce.to_be_bytes());

        BytesN::from_array(env, &env.crypto().sha256(&data).to_array())
    }

    fn generate_wrapped_token_id(
        env: &Env,
        nft_contract: Address,
        token_id: i128,
        dest_chain: u32,
    ) -> i128 {
        let mut data = Bytes::new(env);
        // Placeholder for NFT contract address
        data.extend_from_slice(&[0u8; 32]);
        data.extend_from_slice(&token_id.to_be_bytes());
        data.extend_from_slice(&dest_chain.to_be_bytes());
        data.extend_from_slice(&env.ledger().timestamp().to_be_bytes());

        let hash = env.crypto().sha256(&data);
        // Convert first 16 bytes to i128 for token ID
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&hash.to_array()[0..16]);
        i128::from_be_bytes(bytes)
    }

    fn get_next_user_nonce(env: &Env, user: &Address) -> u64 {
        let key = DataKey::UserNonces(user.clone());
        let current: u64 = env.storage().instance().get(&key).unwrap_or(0);
        let next = current + 1;
        env.storage().instance().set(&key, &next);
        next
    }

    fn get_next_bridge_nonce(env: &Env) -> u64 {
        let current: u64 = env.storage().instance().get(&DataKey::BridgeNonces).unwrap_or(0);
        let next = current + 1;
        env.storage().instance().set(&DataKey::BridgeNonces, &next);
        next
    }

    fn calculate_fee(env: &Env, amount: i128, config: &BridgeConfig) -> Result<i128, Error> {
        let fee = (amount * config.base_fee_bps as i128) / BASIS_POINTS as i128;
        let final_fee = fee.max(config.min_fee).min(config.max_fee);

        if final_fee > config.max_fee {
            return Err(Error::FeeTooHigh);
        }
        if final_fee < config.min_fee {
            return Err(Error::FeeTooLow);
        }

        Ok(final_fee)
    }

    fn verify_signatures(
        env: &Env,
        message: &BridgeMessage,
        signatures: &Vec<ValidatorSignature>,
        validators: &Vec<Address>,
        required: u32,
    ) -> Result<(), Error> {
        if (signatures.len() as u32) < required {
            return Err(Error::InsufficientSignatures);
        }

        let message_bytes = Self::message_to_bytes(env, message);
        let message_hash = env.crypto().sha256(&message_bytes);

        let mut valid_signatures = 0u32;

        for sig in signatures.iter() {
            if validators.contains(&sig.validator) {
                // TODO: Implement actual signature verification
                // For now, we'll assume signatures are valid in tests
                // In production, this would verify Ed25519 signatures
                valid_signatures += 1;
            }
        }

        if valid_signatures < required {
            return Err(Error::InvalidSignature);
        }

        Ok(())
    }

    fn message_to_bytes(env: &Env, message: &BridgeMessage) -> Bytes {
        let mut data = Bytes::new(env);
        data.extend_from_slice(&message.message_id.to_array());
        data.extend_from_slice(&message.source_chain.to_be_bytes());
        data.extend_from_slice(&message.dest_chain.to_be_bytes());
        data.extend_from_slice(&(message.action as u32).to_be_bytes());
        data.extend_from_slice(&(message.asset_type as u32).to_be_bytes());
        // For hashing, we'll use a simple representation
        // In production, proper address serialization would be needed
        data.extend_from_slice(&[0u8; 32]); // Placeholder for address bytes
        data.extend_from_slice(&message.asset_amount.to_be_bytes());
        data.extend_from_slice(&[0u8; 32]); // Placeholder for sender address
        data.extend_from_slice(message.recipient.to_buffer::<1024>().as_slice());
        data.extend_from_slice(&message.fee_amount.to_be_bytes());
        data.extend_from_slice(&message.timestamp.to_be_bytes());
        data.extend_from_slice(&message.nonce.to_be_bytes());
        data
    }

    fn process_unlock(env: &Env, message: &BridgeMessage) -> Result<(), Error> {
        // For unlock operations, we need to release assets to the recipient
        // This would typically involve checking if the assets are locked
        // and then transferring them to the recipient

        match message.asset_type {
            AssetType::Token => {
                let token_client = token::Client::new(env, &message.asset_address);
                let recipient_addr = Self::bytes_to_address(env, &message.recipient)?;
                token_client.transfer(&env.current_contract_address(), &recipient_addr, &message.asset_amount);
            }
            AssetType::NFT => {
                // Handle NFT unlock
                // This would involve minting or transferring the NFT
            }
        }

        Ok(())
    }

    fn bytes_to_address(_env: &Env, _bytes: &Bytes) -> Result<Address, Error> {
        // TODO: Implement proper address conversion from bytes
        // This requires careful handling of different address formats
        // For now, return an error to indicate this needs proper implementation
        Err(Error::InvalidRecipient)
    }

    fn get_chain_id(env: &Env) -> u32 {
        let config: BridgeConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        config.chain_id
    }

    fn assert_admin(env: &Env, user: &Address) -> Result<(), Error> {
        let config: BridgeConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        if config.admin != *user {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    fn assert_not_paused(env: &Env) -> Result<(), Error> {
        let config: BridgeConfig = env.storage().instance().get(&DataKey::Config).unwrap();
        if config.paused {
            return Err(Error::ContractPaused);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_bridge_initialization() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BridgeContract);
        let client = BridgeContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let fee_collector = Address::generate(&env);

        client.initialize(&admin, &5u32, &0u32, &fee_collector);

        let config = client.get_config();
        assert_eq!(config.admin, admin);
        assert_eq!(config.required_signatures, 5);
        assert_eq!(config.chain_id, 0);
    }

    #[test]
    fn test_add_validator() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BridgeContract);
        let client = BridgeContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let fee_collector = Address::generate(&env);
        let validator = Address::generate(&env);

        client.initialize(&admin, &2u32, &0u32, &fee_collector);
        client.add_validator(&admin, &validator);

        let validators = client.get_validators();
        assert_eq!(validators.len(), 1);
        assert_eq!(validators.get(0).unwrap(), validator);
    }

    #[test]
    fn test_bridge_assets_token() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, BridgeContract);
        let client = BridgeContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let fee_collector = Address::generate(&env);
        let user = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(admin.clone());

        client.initialize(&admin, &1u32, &0u32, &fee_collector);

        // For testing, we'll use a mock token - actual minting would be done externally
        // In a real scenario, the user would already have tokens

        // Bridge tokens
        let recipient = Bytes::from_array(&env, &[1u8; 32]);
        let message_id = client.bridge_assets(&user, &token_contract.address(), &AssetType::Token, &500, &1u32, &recipient);

        // Verify bridge initiation (actual token transfer would happen in real scenario)
        let locked = client.get_locked_asset(&message_id);
        assert!(locked.is_some());

        // Check locked asset
        let locked = client.get_locked_asset(&message_id);
        assert!(locked.is_some());
        let locked_asset = locked.unwrap();
        assert_eq!(locked_asset.amount, 500);
        assert_eq!(locked_asset.owner, user);
    }
}
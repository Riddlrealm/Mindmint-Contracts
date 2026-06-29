#![no_std]

mod storage;
mod events;
#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, Address, Env, Bytes, BytesN, panic_with_error,
};
use crate::storage::*;
use crate::events::*;

#[contract]
pub struct DecentralizedIdentityContract;

#[contractimpl]
impl DecentralizedIdentityContract {
    /// Initialize the DID contract
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Initialized) {
            panic!("DID contract already initialized");
        }

        set_admin(&env, &admin);
        env.storage().instance().set(&DataKey::Initialized, &true);

        emit_contract_initialized(&env, admin);
    }

    /// Create a new DID for a user
    pub fn create_did(
        env: Env,
        user: Address,
        public_key: BytesN<32>,
        document: Bytes,
    ) -> BytesN<32> {
        user.require_auth();

        if has_did(&env, &user) {
            panic_with_error!(&env, 1); // DID already exists for this user
        }

        // Generate unique DID identifier
        let did_id = env.crypto().keccak256(&Bytes::from_array(env, &user.to_array()));

        let did = DID {
            id: did_id,
            owner: user.clone(),
            public_key,
            document,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
            active: true,
        };

        set_did(&env, &did_id, &did);
        set_user_did(&env, &user, &did_id);

        emit_did_created(&env, did_id, user);

        did_id
    }

    /// Issue a verifiable credential to a subject
    pub fn issue_credential(
        env: Env,
        issuer: Address,
        subject: Address,
        claim_type: Bytes,
        claims: Bytes,
        expiration: u64,
        privacy_proof: Option<BytesN<32>>,
    ) -> BytesN<32> {
        issuer.require_auth();

        // Verify issuer has a DID
        let issuer_did_id = get_user_did(&env, &issuer).unwrap_or_else(|| {
            panic_with_error!(&env, 2); // Issuer must have a DID
        });

        // Generate credential ID
        let cred_id = env.crypto().keccak256(&Bytes::from_array(env, &subject.to_array()));

        let credential = VerifiableCredential {
            id: cred_id,
            issuer: issuer_did_id,
            subject,
            claim_type,
            claims,
            issued_at: env.ledger().timestamp(),
            expiration,
            revoked: false,
            privacy_proof,
        };

        set_credential(&env, &cred_id, &credential);
        add_user_credential(&env, &subject, &cred_id);

        emit_credential_issued(&env, cred_id, issuer, subject);

        cred_id
    }

    /// Verify a credential is valid
    pub fn verify_credential(env: Env, cred_id: BytesN<32>) -> bool {
        let credential = get_credential(&env, &cred_id).unwrap_or_else(|| {
            panic_with_error!(&env, 3); // Credential not found
        });

        let current_time = env.ledger().timestamp();

        // Check if credential is revoked or expired
        !credential.revoked && credential.expiration > current_time
    }

    /// Add/Update identity attributes
    pub fn update_attributes(env: Env, user: Address, key: Bytes, value: Bytes) {
        user.require_auth();

        let did_id = get_user_did(&env, &user).unwrap_or_else(|| {
            panic_with_error!(&env, 4); // User must have a DID
        });

        let mut did = get_did(&env, &did_id).unwrap();
        did.updated_at = env.ledger().timestamp();
        set_did(&env, &did_id, &did);

        set_attribute(&env, &user, &key, &value);

        emit_attributes_updated(&env, user, key);
    }

    /// Revoke a previously issued credential
    pub fn revoke_credential(env: Env, issuer: Address, cred_id: BytesN<32) {
        issuer.require_auth();

        let mut credential = get_credential(&env, &cred_id).unwrap_or_else(|| {
            panic_with_error!(&env, 5); // Credential not found
        });

        // Verify issuer is the one who issued the credential
        if credential.subject != issuer && !is_delegate(&env, &issuer, &credential.subject) {
            panic_with_error!(&env, 6); // Unauthorized to revoke
        }

        credential.revoked = true;
        set_credential(&env, &cred_id, &credential);

        emit_credential_revoked(&env, cred_id, issuer);
    }

    /// Delegate authority to another address
    pub fn add_delegate(env: Env, user: Address, delegate: Address, permissions: Bytes) {
        user.require_auth();

        if is_delegate(&env, &delegate, &user) {
            panic_with_error!(&env, 7); // Already a delegate
        }

        let delegation = Delegation {
            delegator: user.clone(),
            delegate: delegate.clone(),
            permissions,
            created_at: env.ledger().timestamp(),
            active: true,
        };

        set_delegation(&env, &user, &delegate, &delegation);
        emit_delegation_added(&env, user, delegate);
    }

    /// Remove a delegate
    pub fn remove_delegate(env: Env, user: Address, delegate: Address) {
        user.require_auth();

        if !is_delegate(&env, &delegate, &user) {
            panic_with_error!(&env, 8); // Not a delegate
        }

        let mut delegation = get_delegation(&env, &user, &delegate).unwrap();
        delegation.active = false;
        set_delegation(&env, &user, &delegate, &delegation);

        emit_delegation_removed(&env, user, delegate);
    }

    /// Rotate public key (key management)
    pub fn rotate_public_key(env: Env, user: Address, new_public_key: BytesN<32>) {
        user.require_auth();

        let did_id = get_user_did(&env, &user).unwrap_or_else(|| {
            panic_with_error!(&env, 9); // User must have a DID
        });

        let mut did = get_did(&env, &did_id).unwrap();
        did.public_key = new_public_key;
        did.updated_at = env.ledger().timestamp();
        set_did(&env, &did_id, &did);

        emit_key_rotated(&env, user);
    }

    /// Deactivate a DID
    pub fn deactivate_did(env: Env, user: Address) {
        user.require_auth();

        let did_id = get_user_did(&env, &user).unwrap_or_else(|| {
            panic_with_error!(&env, 10); // User must have a DID
        });

        let mut did = get_did(&env, &did_id).unwrap();
        did.active = false;
        did.updated_at = env.ledger().timestamp();
        set_did(&env, &did_id, &did);

        emit_did_deactivated(&env, user);
    }

    /// Verify a privacy-preserving proof
    pub fn verify_privacy_proof(env: Env, cred_id: BytesN<32>, proof: BytesN<32>) -> bool {
        let credential = get_credential(&env, &cred_id).unwrap_or_else(|| {
            panic_with_error!(&env, 11); // Credential not found
        });

        match credential.privacy_proof {
            Some(stored_proof) => proof == stored_proof && !credential.revoked,
            None => false,
        }
    }

    // View functions
    pub fn get_did(env: Env, did_id: BytesN<32>) -> Option<DID> {
        get_did(&env, &did_id)
    }

    pub fn get_user_did(env: Env, user: Address) -> Option<BytesN<32>> {
        if !has_did(&env, &user) {
            None
        } else {
            Some(get_user_did(&env, &user).unwrap())
        }
    }

    pub fn get_credential(env: Env, cred_id: BytesN<32>) -> Option<VerifiableCredential> {
        get_credential(&env, &cred_id)
    }

    pub fn get_attribute(env: Env, user: Address, key: Bytes) -> Option<Bytes> {
        if !has_attribute(&env, &user, &key) {
            None
        } else {
            Some(get_attribute(&env, &user, &key).unwrap())
        }
    }

    pub fn get_user_credentials(env: Env, user: Address) -> Vec<BytesN<32>> {
        get_user_credentials(&env, &user)
    }

    pub fn is_delegate(env: Env, delegate: Address, delegator: Address) -> bool {
        is_delegate(&env, &delegate, &delegator)
    }

    pub fn get_credential_status(env: Env, cred_id: BytesN<32>) -> CredentialStatus {
        let cred = get_credential(&env, &cred_id).unwrap_or_else(|| {
            panic_with_error!(&env, 12); // Credential not found
        });

        let current_time = env.ledger().timestamp();
        CredentialStatus {
            revoked: cred.revoked,
            expired: cred.expiration < current_time,
            is_valid: !cred.revoked && cred.expiration > current_time,
        }
    }
}

#[contracttype]
pub struct CredentialStatus {
    pub revoked: bool,
    pub expired: bool,
    pub is_valid: bool,
}
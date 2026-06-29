use super::*;
use soroban_sdk::{
    testutils::Address as _,
    Address, Env, Bytes, BytesN,
};

#[test]
fn test_initialize_contract() {
    let env = Env::default();
    let admin = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    // Verify admin is set correctly by trying to create a DID
    let user = Address::generate(&env);
    let public_key = BytesN::from_array(&env, &[1; 32]);
    let document = Bytes::from_slice(&env, b"DID Document");
    
    let did_id = client.create_did(&user, &public_key, &document);
    
    assert!(client.get_did(&did_id).is_some());
}

#[test]
fn test_create_did() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    let public_key = BytesN::from_array(&env, &[1; 32]);
    let document = Bytes::from_slice(&env, b"DID Document");
    
    let did_id = client.create_did(&user, &public_key, &document);
    
    let did = client.get_did(&did_id).unwrap();
    assert_eq!(did.owner, user);
    assert!(did.active);
    assert_eq!(client.get_user_did(&user).unwrap(), did_id);
}

#[test]
#[should_panic(expected = "code = 1")]
fn test_cannot_create_duplicate_did() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    let public_key = BytesN::from_array(&env, &[1; 32]);
    let document = Bytes::from_slice(&env, b"DID Document");
    
    // Create first DID - should work
    client.create_did(&user, &public_key, &document);
    
    // Try to create second DID for same user - should panic
    client.create_did(&user, &public_key, &document);
}

#[test]
fn test_issue_and_verify_credential() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let subject = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    // Create DIDs for issuer and subject
    let pk = BytesN::from_array(&env, &[1; 32]);
    let doc = Bytes::from_slice(&env, b"doc");
    client.create_did(&issuer, &pk, &doc);
    client.create_did(&subject, &pk, &doc);
    
    // Issue a credential that expires in 1 year
    let claim_type = Bytes::from_slice(&env, b"IdentityVerification");
    let claims = Bytes::from_slice(&env, b"{\"verified\":true}");
    let expiration = env.ledger().timestamp() + 31557600;
    
    let cred_id = client.issue_credential(
        &issuer,
        &subject,
        &claim_type,
        &claims,
        &expiration,
        None,
    );
    
    // Verify credential is valid
    assert!(client.verify_credential(&cred_id));
    
    let status = client.get_credential_status(&cred_id);
    assert!(status.is_valid);
    assert!(!status.revoked);
    assert!(!status.expired);
}

#[test]
fn test_revoke_credential() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let subject = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    // Create DIDs
    let pk = BytesN::from_array(&env, &[1; 32]);
    let doc = Bytes::from_slice(&env, b"doc");
    client.create_did(&issuer, &pk, &doc);
    client.create_did(&subject, &pk, &doc);
    
    // Issue credential
    let claim_type = Bytes::from_slice(&env, b"test");
    let claims = Bytes::from_slice(&env, b"{}");
    let expiration = env.ledger().timestamp() + 31557600;
    
    let cred_id = client.issue_credential(
        &issuer,
        &subject,
        &claim_type,
        &claims,
        &expiration,
        None,
    );
    
    // Revoke the credential
    client.revoke_credential(&issuer, &cred_id);
    
    // Verify it's no longer valid
    assert!(!client.verify_credential(&cred_id));
    
    let status = client.get_credential_status(&cred_id);
    assert!(status.revoked);
    assert!(!status.is_valid);
}

#[test]
fn test_update_attributes() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    // Create DID
    let pk = BytesN::from_array(&env, &[1; 32]);
    let doc = Bytes::from_slice(&env, b"doc");
    client.create_did(&user, &pk, &doc);
    
    // Add an attribute
    let key = Bytes::from_slice(&env, b"username");
    let value = Bytes::from_slice(&env, b"alice123");
    client.update_attributes(&user, &key, &value);
    
    // Retrieve the attribute
    let stored_value = client.get_attribute(&user, &key).unwrap();
    assert_eq!(stored_value, value);
}

#[test]
fn test_delegation() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let delegate = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    // Create DID for user
    let pk = BytesN::from_array(&env, &[1; 32]);
    let doc = Bytes::from_slice(&env, b"doc");
    client.create_did(&user, &pk, &doc);
    
    // Add delegate
    let permissions = Bytes::from_slice(&env, b"can_issue_credentials");
    client.add_delegate(&user, &delegate, &permissions);
    
    assert!(client.is_delegate(&delegate, &user));
    
    // Remove delegate
    client.remove_delegate(&user, &delegate);
    assert!(!client.is_delegate(&delegate, &user));
}

#[test]
fn test_rotate_public_key() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    // Create DID with initial key
    let old_key = BytesN::from_array(&env, &[1; 32]);
    let doc = Bytes::from_slice(&env, b"doc");
    let did_id = client.create_did(&user, &old_key, &doc);
    
    // Rotate to new key
    let new_key = BytesN::from_array(&env, &[2; 32]);
    client.rotate_public_key(&user, &new_key);
    
    // Verify key was updated
    let did = client.get_did(&did_id).unwrap();
    assert_eq!(did.public_key, new_key);
}

#[test]
fn test_deactivate_did() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    // Create DID
    let pk = BytesN::from_array(&env, &[1; 32]);
    let doc = Bytes::from_slice(&env, b"doc");
    let did_id = client.create_did(&user, &pk, &doc);
    
    // Deactivate
    client.deactivate_did(&user);
    
    // Verify it's inactive
    let did = client.get_did(&did_id).unwrap();
    assert!(!did.active);
}

#[test]
fn test_privacy_proof_verification() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let subject = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    // Create DIDs
    let pk = BytesN::from_array(&env, &[1; 32]);
    let doc = Bytes::from_slice(&env, b"doc");
    client.create_did(&issuer, &pk, &doc);
    client.create_did(&subject, &pk, &doc);
    
    // Issue credential with privacy proof
    let proof = BytesN::from_array(&env, &[5; 32]);
    let claim_type = Bytes::from_slice(&env, b"age_verification");
    let claims = Bytes::from_slice(&env, b"{\"age\":18}");
    let expiration = env.ledger().timestamp() + 31557600;
    
    let cred_id = client.issue_credential(
        &issuer,
        &subject,
        &claim_type,
        &claims,
        &expiration,
        Some(proof),
    );
    
    // Verify proof
    assert!(client.verify_privacy_proof(&cred_id, &proof));
}

#[test]
fn test_get_user_credentials() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let subject = Address::generate(&env);
    
    let contract = env.register(DecentralizedIdentityContract, ());
    let client = DecentralizedIdentityPoolClient::new(&env, &contract);
    
    client.initialize(&admin);
    
    // Create DIDs
    let pk = BytesN::from_array(&env, &[1; 32]);
    let doc = Bytes::from_slice(&env, b"doc");
    client.create_did(&issuer, &pk, &doc);
    client.create_did(&subject, &pk, &doc);
    
    // Issue multiple credentials
    for i in 0..3 {
        let claim_type = Bytes::from_slice(&env, &format!("type{}", i).as_bytes());
        let claims = Bytes::from_slice(&env, b"{}");
        let expiration = env.ledger().timestamp() + 31557600;
        
        client.issue_credential(
            &issuer,
            &subject,
            &claim_type,
            &claims,
            &expiration,
            None,
        );
    }
    
    let credentials = client.get_user_credentials(&subject);
    assert_eq!(credentials.len(), 3);
}
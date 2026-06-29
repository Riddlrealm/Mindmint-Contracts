use soroban_sdk::{Env, Address, BytesN, symbol_short};

pub(crate) fn emit_contract_initialized(
    env: &Env,
    admin: Address,
) {
    env.events().publish(
        (symbol_short!("init"),),
        admin,
    );
}

pub(crate) fn emit_did_created(
    env: &Env,
    did_id: BytesN<32>,
    owner: Address,
) {
    env.events().publish(
        (symbol_short!("did_create"), owner),
        did_id,
    );
}

pub(crate) fn emit_did_deactivated(
    env: &Env,
    owner: Address,
) {
    env.events().publish(
        (symbol_short!("did_deactivate"), owner),
        (),
    );
}

pub(crate) fn emit_credential_issued(
    env: &Env,
    cred_id: BytesN<32>,
    issuer: Address,
    subject: Address,
) {
    env.events().publish(
        (symbol_short!("cred_issue"), issuer, subject),
        cred_id,
    );
}

pub(crate) fn emit_credential_revoked(
    env: &Env,
    cred_id: BytesN<32>,
    issuer: Address,
) {
    env.events().publish(
        (symbol_short!("cred_revoke"), issuer),
        cred_id,
    );
}

pub(crate) fn emit_attributes_updated(
    env: &Env,
    user: Address,
    key: Bytes,
) {
    env.events().publish(
        (symbol_short!("attr_update"), user),
        key,
    );
}

pub(crate) fn emit_delegation_added(
    env: &Env,
    delegator: Address,
    delegate: Address,
) {
    env.events().publish(
        (symbol_short!("delegate_add"), delegator),
        delegate,
    );
}

pub(crate) fn emit_delegation_removed(
    env: &Env,
    delegator: Address,
    delegate: Address,
) {
    env.events().publish(
        (symbol_short!("delegate_remove"), delegator),
        delegate,
    );
}

pub(crate) fn emit_key_rotated(
    env: &Env,
    user: Address,
) {
    env.events().publish(
        (symbol_short!("key_rotate"), user),
        (),
    );
}
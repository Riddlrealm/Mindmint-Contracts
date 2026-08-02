use crate::types::{AuditEntry, RBACError};
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract admin address (instance storage).
    Admin,
    /// Emergency admin address (instance storage).
    EmergencyAdmin,
    /// Global pause flag (instance storage).
    Paused,
    /// Per-account role set, keyed by account address (persistent storage).
    UserRoles(Address),
    /// Per-role permission set, keyed by role name (persistent storage).
    RolePermissions(Symbol),
    /// Role hierarchy parent link, keyed by child role (persistent storage).
    RoleParent(Symbol),
    /// Audit log of role/permission changes (instance storage).
    AuditLogs,
}

pub struct Storage;

impl Storage {
    pub fn has_admin(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Admin)
    }

    pub fn set_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&DataKey::Admin, admin);
    }

    pub fn get_admin(env: &Env) -> Result<Address, RBACError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(RBACError::NotInitialized)
    }

    pub fn set_emergency_admin(env: &Env, admin: &Address) {
        env.storage()
            .instance()
            .set(&DataKey::EmergencyAdmin, admin);
    }

    pub fn get_emergency_admin(env: &Env) -> Result<Address, RBACError> {
        env.storage()
            .instance()
            .get(&DataKey::EmergencyAdmin)
            .ok_or(RBACError::NotInitialized)
    }

    pub fn set_paused(env: &Env, paused: bool) {
        env.storage()
            .instance()
            .set(&DataKey::Paused, &paused);
    }

    pub fn get_paused(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn set_user_roles(env: &Env, address: &Address, roles: &Vec<Symbol>) {
        env.storage()
            .persistent()
            .set(&DataKey::UserRoles(address.clone()), roles);
    }

    pub fn get_user_roles(env: &Env, address: &Address) -> Vec<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::UserRoles(address.clone()))
            .unwrap_or_else(|| Vec::new(env))
    }

    pub fn set_role_permissions(env: &Env, role: &Symbol, permissions: &Vec<Symbol>) {
        env.storage()
            .persistent()
            .set(&DataKey::RolePermissions(role.clone()), permissions);
    }

    pub fn get_role_permissions(env: &Env, role: &Symbol) -> Vec<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::RolePermissions(role.clone()))
            .unwrap_or_else(|| Vec::new(env))
    }

    pub fn set_role_parent(env: &Env, child: &Symbol, parent: &Symbol) {
        env.storage()
            .persistent()
            .set(&DataKey::RoleParent(child.clone()), parent);
    }

    pub fn get_role_parent(env: &Env, child: &Symbol) -> Option<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::RoleParent(child.clone()))
    }

    pub fn remove_role_parent(env: &Env, child: &Symbol) {
        env.storage()
            .persistent()
            .remove(&DataKey::RoleParent(child.clone()));
    }

    pub fn get_audit_logs(env: &Env) -> Vec<AuditEntry> {
        env.storage()
            .instance()
            .get(&DataKey::AuditLogs)
            .unwrap_or_else(|| Vec::new(env))
    }

    pub fn set_audit_logs(env: &Env, logs: &Vec<AuditEntry>) {
        env.storage().instance().set(&DataKey::AuditLogs, logs);
    }

    pub fn add_audit_entry(
        env: &Env,
        action: Symbol,
        caller: &Address,
        target: Option<Address>,
        role: Symbol,
        permission: Option<Symbol>,
    ) {
        let mut logs = Self::get_audit_logs(env);
        let entry = AuditEntry {
            timestamp: env.ledger().timestamp(),
            action,
            caller: caller.clone(),
            target,
            role,
            permission,
        };
        logs.push_back(entry);
        Self::set_audit_logs(env, &logs);
    }
}

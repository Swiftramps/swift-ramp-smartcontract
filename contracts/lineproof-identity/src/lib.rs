#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, Symbol,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityStatus {
    Active,
    Revoked,
    Suspended,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityRecord {
    pub status: IdentityStatus,
    pub queue_id: Option<Symbol>,
    pub registered_at: u64,
}

#[contracttype]
pub enum DataKey {
    IdentityRecord(Address),
    QueueMembership(Symbol, Address),
}

#[contract]
pub struct LineproofIdentity;

#[contractimpl]
impl LineproofIdentity {
    pub fn initialize(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::IdentityRecord(admin.clone()), &IdentityRecord {
            status: IdentityStatus::Active,
            queue_id: None,
            registered_at: env.ledger().sequence() as u64,
        });
    }

    pub fn register_identity(env: Env, admin: Address, user: Address, queue_id: Option<Symbol>) {
        let admin_record: IdentityRecord = env.storage().instance().get(&DataKey::IdentityRecord(admin.clone())).unwrap();
        if admin_record.status != IdentityStatus::Active {
            panic!("admin not authorized");
        }
        admin.require_auth();

        let identity_record = IdentityRecord {
            status: IdentityStatus::Active,
            queue_id: queue_id.clone(),
            registered_at: env.ledger().sequence() as u64,
        };
        env.storage().instance().set(&DataKey::IdentityRecord(user.clone()), &identity_record);

        if let Some(qid) = queue_id {
            env.storage().instance().set(&DataKey::QueueMembership(qid, user), &true);
        }
    }

    pub fn revoke_identity(env: Env, admin: Address, user: Address) {
        let admin_record: IdentityRecord = env.storage().instance().get(&DataKey::IdentityRecord(admin.clone())).unwrap();
        if admin_record.status != IdentityStatus::Active {
            panic!("admin not authorized");
        }
        admin.require_auth();

        let mut identity_record: IdentityRecord = env.storage().instance().get(&DataKey::IdentityRecord(user.clone())).unwrap();
        identity_record.status = IdentityStatus::Revoked;
        env.storage().instance().set(&DataKey::IdentityRecord(user), &identity_record);
    }

    /// Cancel an enrollment and remove every storage entry it created.
    ///
    /// `register_identity` writes two entries: the `IdentityRecord` and, when a
    /// queue was supplied, a `QueueMembership` flag. Removing only one of them
    /// leaves the other as stale data — a `QueueMembership` with no identity
    /// behind it, or an identity still claiming a queue it is no longer in. This
    /// removes both, reading the queue back off the record so the caller cannot
    /// pass a mismatched `queue_id` and orphan the membership entry.
    ///
    /// Cleanup is **eager**: the entries are deleted in this call rather than
    /// being tombstoned and swept later. See `docs/storage-cleanup.md` for why.
    ///
    /// Idempotent: cancelling an identity that is already gone is a no-op rather
    /// than a panic, so a retried transaction cannot fail on the second attempt.
    pub fn cancel(env: Env, admin: Address, user: Address) {
        let admin_record: IdentityRecord = env
            .storage()
            .instance()
            .get(&DataKey::IdentityRecord(admin.clone()))
            .unwrap();
        if admin_record.status != IdentityStatus::Active {
            panic!("admin not authorized");
        }
        admin.require_auth();

        let record: Option<IdentityRecord> = env
            .storage()
            .instance()
            .get(&DataKey::IdentityRecord(user.clone()));

        // Nothing enrolled means nothing to clean up.
        let record = match record {
            Some(record) => record,
            None => return,
        };

        // Remove the membership first, using the queue recorded at registration.
        if let Some(queue_id) = record.queue_id {
            env.storage()
                .instance()
                .remove(&DataKey::QueueMembership(queue_id, user.clone()));
        }

        env.storage()
            .instance()
            .remove(&DataKey::IdentityRecord(user));
    }

    pub fn can_transfer(env: Env, from: Address, to: Address, queue_id: Symbol) -> bool {
        // If from == to, transfer is always allowed (self-transfer)
        if from == to {
            return true;
        }

        // CRITICAL: Read the IdentityRecord from storage to check status
        // Default values for unregistered identities should be treated as invalid
        let from_record: Option<IdentityRecord> = env.storage().instance().get(&DataKey::IdentityRecord(from.clone()));
        let to_record: Option<IdentityRecord> = env.storage().instance().get(&DataKey::IdentityRecord(to.clone()));

        // Unregistered identities cannot transfer
        let from_identity = match from_record {
            Some(record) => record,
            None => return false,
        };

        let to_identity = match to_record {
            Some(record) => record,
            None => return false,
        };

        // CRITICAL: Return false for revoked identity regardless of queue membership
        if from_identity.status == IdentityStatus::Revoked {
            return false;
        }

        if to_identity.status == IdentityStatus::Revoked {
            return false;
        }

        // Verify queue membership for both parties
        let from_in_queue: Option<bool> = env.storage().instance().get(&DataKey::QueueMembership(queue_id.clone(), from.clone()));
        let to_in_queue: Option<bool> = env.storage().instance().get(&DataKey::QueueMembership(queue_id, to.clone()));

        // Both parties must be in the queue
        from_in_queue.unwrap_or(false) && to_in_queue.unwrap_or(false)
    }

    pub fn get_identity_status(env: Env, user: Address) -> IdentityStatus {
        let record: Option<IdentityRecord> = env.storage().instance().get(&DataKey::IdentityRecord(user));
        match record {
            Some(identity) => identity.status,
            None => panic!("identity not found"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Env};

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        LineproofIdentityClient::new(&env, &contract_id).initialize(&admin);
    }

    #[test]
    fn test_can_transfer_reads_identity_record() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Register both users in the queue
        client.register_identity(&admin, &user1, &Some(queue_id.clone()));
        client.register_identity(&admin, &user2, &Some(queue_id.clone()));

        // Active identities in queue should be able to transfer
        assert!(client.can_transfer(&user1, &user2, &queue_id));
        assert!(client.can_transfer(&user2, &user1, &queue_id));
    }

    #[test]
    fn test_can_transfer_revoked_identity() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Register both users in the queue
        client.register_identity(&admin, &user1, &Some(queue_id.clone()));
        client.register_identity(&admin, &user2, &Some(queue_id.clone()));

        // Revoke user1
        client.revoke_identity(&admin, &user1);

        // Revoked identity cannot transfer even if in queue
        assert!(!client.can_transfer(&user1, &user2, &queue_id));
        assert!(!client.can_transfer(&user2, &user1, &queue_id));
    }

    #[test]
    fn test_can_transfer_unregistered_identity() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Register only user1
        client.register_identity(&admin, &user1, &Some(queue_id.clone()));

        // Unregistered user2 cannot transfer
        assert!(!client.can_transfer(&user1, &user2, &queue_id));
        assert!(!client.can_transfer(&user2, &user1, &queue_id));
    }

    #[test]
    fn test_can_transfer_not_in_queue() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Register both users without queue
        client.register_identity(&admin, &user1, &None);
        client.register_identity(&admin, &user2, &None);

        // Users not in queue cannot transfer
        assert!(!client.can_transfer(&user1, &user2, &queue_id));
    }

    #[test]
    fn test_can_transfer_self_transfer() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Self-transfer should always be allowed
        assert!(client.can_transfer(&user1, &user1, &queue_id));
    }

    #[test]
    fn test_get_identity_status() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        client.register_identity(&admin, &user1, &Some(queue_id.clone()));
        assert_eq!(client.get_identity_status(&user1), IdentityStatus::Active);

        client.revoke_identity(&admin, &user1);
        assert_eq!(client.get_identity_status(&user1), IdentityStatus::Revoked);
    }

    // ---- cancel(): storage cleanup (issue #13) ----------------------------

    /// Read raw storage from inside the contract context, so the assertions are
    /// about what is actually persisted rather than what a getter reports.
    fn has_identity(env: &Env, contract_id: &Address, user: &Address) -> bool {
        env.as_contract(contract_id, || {
            env.storage()
                .instance()
                .has(&DataKey::IdentityRecord(user.clone()))
        })
    }

    fn has_membership(env: &Env, contract_id: &Address, queue: &Symbol, user: &Address) -> bool {
        env.as_contract(contract_id, || {
            env.storage()
                .instance()
                .has(&DataKey::QueueMembership(queue.clone(), user.clone()))
        })
    }

    #[test]
    fn test_cancel_removes_identity_and_membership() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);
        client.register_identity(&admin, &user, &Some(queue_id.clone()));

        // Both entries exist before cancelling.
        assert!(has_identity(&env, &contract_id, &user));
        assert!(has_membership(&env, &contract_id, &queue_id, &user));

        client.cancel(&admin, &user);

        // Neither entry survives: no stale membership, no orphaned identity.
        assert!(!has_identity(&env, &contract_id, &user), "identity record left behind");
        assert!(!has_membership(&env, &contract_id, &queue_id, &user), "queue membership left behind");
    }

    #[test]
    fn test_cancel_without_queue_removes_identity() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);
        client.register_identity(&admin, &user, &None);

        assert!(has_identity(&env, &contract_id, &user));
        client.cancel(&admin, &user);
        assert!(!has_identity(&env, &contract_id, &user));
    }

    #[test]
    fn test_cancel_leaves_other_users_untouched() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);
        client.register_identity(&admin, &user1, &Some(queue_id.clone()));
        client.register_identity(&admin, &user2, &Some(queue_id.clone()));

        client.cancel(&admin, &user1);

        // Cleanup is scoped to the cancelled user only.
        assert!(!has_identity(&env, &contract_id, &user1));
        assert!(!has_membership(&env, &contract_id, &queue_id, &user1));
        assert!(has_identity(&env, &contract_id, &user2));
        assert!(has_membership(&env, &contract_id, &queue_id, &user2));
        // ...and the admin's own record is untouched.
        assert!(has_identity(&env, &contract_id, &admin));
    }

    #[test]
    fn test_cancel_is_idempotent() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);
        client.register_identity(&admin, &user, &Some(queue_id.clone()));

        // A retried transaction must not fail on the second attempt.
        client.cancel(&admin, &user);
        client.cancel(&admin, &user);
        assert!(!has_identity(&env, &contract_id, &user));
    }

    #[test]
    fn test_cancel_of_unknown_user_is_a_noop() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let stranger = Address::generate(&env);

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        client.cancel(&admin, &stranger);
        assert!(!has_identity(&env, &contract_id, &stranger));
    }

    #[test]
    fn test_cancelled_user_cannot_transfer() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);
        client.register_identity(&admin, &user1, &Some(queue_id.clone()));
        client.register_identity(&admin, &user2, &Some(queue_id.clone()));
        assert!(client.can_transfer(&user1, &user2, &queue_id));

        client.cancel(&admin, &user1);

        // Removal must read as "unregistered", not as a lingering member.
        assert!(!client.can_transfer(&user1, &user2, &queue_id));
        assert!(!client.can_transfer(&user2, &user1, &queue_id));
    }

    #[test]
    fn test_cancel_then_reregister_restores_access() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);
        client.register_identity(&admin, &user1, &Some(queue_id.clone()));
        client.register_identity(&admin, &user2, &Some(queue_id.clone()));

        client.cancel(&admin, &user1);
        client.register_identity(&admin, &user1, &Some(queue_id.clone()));

        // Eager deletion must not leave anything that blocks re-enrollment.
        assert!(has_membership(&env, &contract_id, &queue_id, &user1));
        assert!(client.can_transfer(&user1, &user2, &queue_id));
    }

}

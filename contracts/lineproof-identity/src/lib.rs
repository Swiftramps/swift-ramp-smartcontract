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
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        LineproofIdentityClient::new(&env, &contract_id).initialize(&admin);
    }

    #[test]
    fn test_can_transfer_reads_identity_record() {
        let env = Env::default();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Register both users in the queue
        client.register_identity(&admin, &user1, Some(queue_id));
        client.register_identity(&admin, &user2, Some(queue_id));

        // Active identities in queue should be able to transfer
        assert!(client.can_transfer(&user1, &user2, queue_id));
        assert!(client.can_transfer(&user2, &user1, queue_id));
    }

    #[test]
    fn test_can_transfer_revoked_identity() {
        let env = Env::default();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Register both users in the queue
        client.register_identity(&admin, &user1, Some(queue_id));
        client.register_identity(&admin, &user2, Some(queue_id));

        // Revoke user1
        client.revoke_identity(&admin, &user1);

        // Revoked identity cannot transfer even if in queue
        assert!(!client.can_transfer(&user1, &user2, queue_id));
        assert!(!client.can_transfer(&user2, &user1, queue_id));
    }

    #[test]
    fn test_can_transfer_unregistered_identity() {
        let env = Env::default();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Register only user1
        client.register_identity(&admin, &user1, Some(queue_id));

        // Unregistered user2 cannot transfer
        assert!(!client.can_transfer(&user1, &user2, queue_id));
        assert!(!client.can_transfer(&user2, &user1, queue_id));
    }

    #[test]
    fn test_can_transfer_not_in_queue() {
        let env = Env::default();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Register both users without queue
        client.register_identity(&admin, &user1, None);
        client.register_identity(&admin, &user2, None);

        // Users not in queue cannot transfer
        assert!(!client.can_transfer(&user1, &user2, queue_id));
    }

    #[test]
    fn test_can_transfer_self_transfer() {
        let env = Env::default();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Self-transfer should always be allowed
        assert!(client.can_transfer(&user1, &user1, queue_id));
    }

    #[test]
    fn test_get_identity_status() {
        let env = Env::default();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let queue_id = symbol_short!("queue1");

        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        client.register_identity(&admin, &user1, Some(queue_id));
        assert_eq!(client.get_identity_status(&user1), IdentityStatus::Active);

        client.revoke_identity(&admin, &user1);
        assert_eq!(client.get_identity_status(&user1), IdentityStatus::Revoked);
    }
}

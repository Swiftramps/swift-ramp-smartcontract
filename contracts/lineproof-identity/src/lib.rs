#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env,
    Symbol,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AdminNotFound = 1,
    AdminNotActive = 2,
    IdentityNotFound = 3,
    AlreadyRevoked = 4,
    AlreadyRegistered = 5,
    NotAuthorized = 6,
}

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
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env
            .storage()
            .instance()
            .has(&DataKey::IdentityRecord(admin.clone()))
        {
            return Err(Error::AlreadyRegistered);
        }
        env.storage().instance().set(
            &DataKey::IdentityRecord(admin.clone()),
            &IdentityRecord {
                status: IdentityStatus::Active,
                queue_id: None,
                registered_at: env.ledger().sequence() as u64,
            },
        );
        Ok(())
    }

    pub fn register_identity(
        env: Env,
        admin: Address,
        user: Address,
        queue_id: Option<Symbol>,
    ) -> Result<(), Error> {
        let admin_record: IdentityRecord = env
            .storage()
            .instance()
            .get(&DataKey::IdentityRecord(admin.clone()))
            .ok_or(Error::AdminNotFound)?;

        if admin_record.status != IdentityStatus::Active {
            return Err(Error::AdminNotActive);
        }
        admin.require_auth();

        if env
            .storage()
            .instance()
            .has(&DataKey::IdentityRecord(user.clone()))
        {
            return Err(Error::AlreadyRegistered);
        }

        let identity_record = IdentityRecord {
            status: IdentityStatus::Active,
            queue_id: queue_id.clone(),
            registered_at: env.ledger().sequence() as u64,
        };
        env.storage()
            .instance()
            .set(&DataKey::IdentityRecord(user.clone()), &identity_record);

        if let Some(qid) = queue_id {
            env.storage()
                .instance()
                .set(&DataKey::QueueMembership(qid, user), &true);
        }
        Ok(())
    }

    pub fn revoke_identity(env: Env, admin: Address, user: Address) -> Result<(), Error> {
        let admin_record: IdentityRecord = env
            .storage()
            .instance()
            .get(&DataKey::IdentityRecord(admin.clone()))
            .ok_or(Error::AdminNotFound)?;

        if admin_record.status != IdentityStatus::Active {
            return Err(Error::AdminNotActive);
        }
        admin.require_auth();

        let mut identity_record: IdentityRecord = env
            .storage()
            .instance()
            .get(&DataKey::IdentityRecord(user.clone()))
            .ok_or(Error::IdentityNotFound)?;

        if identity_record.status == IdentityStatus::Revoked {
            return Err(Error::AlreadyRevoked);
        }

        identity_record.status = IdentityStatus::Revoked;
        env.storage()
            .instance()
            .set(&DataKey::IdentityRecord(user), &identity_record);
        Ok(())
    }

    pub fn can_transfer(env: Env, from: Address, to: Address, queue_id: Symbol) -> bool {
        // If from == to, transfer is always allowed (self-transfer)
        if from == to {
            return true;
        }

        let from_record: Option<IdentityRecord> = env
            .storage()
            .instance()
            .get(&DataKey::IdentityRecord(from.clone()));
        let to_record: Option<IdentityRecord> = env
            .storage()
            .instance()
            .get(&DataKey::IdentityRecord(to.clone()));

        // Unregistered identities cannot transfer
        let from_identity = match from_record {
            Some(record) => record,
            None => return false,
        };

        let to_identity = match to_record {
            Some(record) => record,
            None => return false,
        };

        // Return false for revoked identity regardless of queue membership
        if from_identity.status == IdentityStatus::Revoked
            || to_identity.status == IdentityStatus::Revoked
        {
            return false;
        }

        // Verify queue membership for both parties
        let from_in_queue: Option<bool> = env
            .storage()
            .instance()
            .get(&DataKey::QueueMembership(queue_id.clone(), from.clone()));
        let to_in_queue: Option<bool> = env
            .storage()
            .instance()
            .get(&DataKey::QueueMembership(queue_id, to.clone()));

        // Both parties must be in the queue
        from_in_queue.unwrap_or(false) && to_in_queue.unwrap_or(false)
    }

    pub fn get_identity_status(env: Env, user: Address) -> Result<IdentityStatus, Error> {
        let record: Option<IdentityRecord> =
            env.storage().instance().get(&DataKey::IdentityRecord(user));
        match record {
            Some(identity) => Ok(identity.status),
            None => Err(Error::IdentityNotFound),
        }
    }

    pub fn compute_proof_hash(
        env: Env,
        identity: Address,
        queue_id: Symbol,
        timestamp: u64,
    ) -> BytesN<32> {
        let mut bytes = Bytes::new(&env);
        bytes.append(&identity.to_xdr(&env));
        bytes.append(&queue_id.to_xdr(&env));
        for b in timestamp.to_be_bytes() {
            bytes.push_back(b);
        }
        env.crypto().sha256(&bytes).into()
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
        let client = LineproofIdentityClient::new(&env, &contract_id);
        assert_eq!(client.initialize(&admin), ());
    }

    #[test]
    fn test_try_initialize_already_registered() {
        let env = Env::default();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let client = LineproofIdentityClient::new(&env, &contract_id);
        client.initialize(&admin);

        let res = client.try_initialize(&admin);
        assert_eq!(res, Err(Ok(Error::AlreadyRegistered)));
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

        client.register_identity(&admin, &user1, &Some(queue_id));
        client.register_identity(&admin, &user2, &Some(queue_id));

        assert!(client.can_transfer(&user1, &user2, &queue_id));
        assert!(client.can_transfer(&user2, &user1, &queue_id));
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

        client.register_identity(&admin, &user1, &Some(queue_id));
        client.register_identity(&admin, &user2, &Some(queue_id));

        client.revoke_identity(&admin, &user1);

        assert!(!client.can_transfer(&user1, &user2, &queue_id));
        assert!(!client.can_transfer(&user2, &user1, &queue_id));
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

        client.register_identity(&admin, &user1, &Some(queue_id));

        assert!(!client.can_transfer(&user1, &user2, &queue_id));
        assert!(!client.can_transfer(&user2, &user1, &queue_id));
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

        client.register_identity(&admin, &user1, &None);
        client.register_identity(&admin, &user2, &None);

        assert!(!client.can_transfer(&user1, &user2, &queue_id));
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

        assert!(client.can_transfer(&user1, &user1, &queue_id));
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

        client.register_identity(&admin, &user1, &Some(queue_id));
        assert_eq!(client.get_identity_status(&user1), IdentityStatus::Active);

        client.revoke_identity(&admin, &user1);
        assert_eq!(client.get_identity_status(&user1), IdentityStatus::Revoked);
    }

    #[test]
    fn test_typed_errors() {
        let env = Env::default();
        let contract_id = env.register(LineproofIdentity, ());
        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let unreg_user = Address::generate(&env);

        let client = LineproofIdentityClient::new(&env, &contract_id);

        // AdminNotFound
        let res_unreg_admin = client.try_register_identity(&unreg_user, &user1, &None);
        assert_eq!(res_unreg_admin, Err(Ok(Error::AdminNotFound)));

        client.initialize(&admin);

        // IdentityNotFound for get_identity_status
        let res_status = client.try_get_identity_status(&unreg_user);
        assert_eq!(res_status, Err(Ok(Error::IdentityNotFound)));

        // IdentityNotFound for revoke_identity
        let res_revoke_unreg = client.try_revoke_identity(&admin, &unreg_user);
        assert_eq!(res_revoke_unreg, Err(Ok(Error::IdentityNotFound)));

        // Register user1 successfully
        client.register_identity(&admin, &user1, &None);

        // AlreadyRegistered for user1
        let res_already_reg = client.try_register_identity(&admin, &user1, &None);
        assert_eq!(res_already_reg, Err(Ok(Error::AlreadyRegistered)));

        // Revoke user1 successfully
        client.revoke_identity(&admin, &user1);

        // AlreadyRevoked for user1
        let res_already_rev = client.try_revoke_identity(&admin, &user1);
        assert_eq!(res_already_rev, Err(Ok(Error::AlreadyRevoked)));

        // AdminNotActive: Revoked admin cannot register new identities
        let res_admin_revoked = client.try_register_identity(&user1, &unreg_user, &None);
        assert_eq!(res_admin_revoked, Err(Ok(Error::AdminNotActive)));
    }

    #[test]
    fn test_proof_hash_determinism() {
        let env = Env::default();
        let contract_id = env.register(LineproofIdentity, ());
        let client = LineproofIdentityClient::new(&env, &contract_id);

        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let user3 = Address::generate(&env);
        let queue_id = symbol_short!("queueA");
        let timestamp = 1700000000u64;

        // 1. Same identity + queue + timestamp -> same hash
        let hash1_a = client.compute_proof_hash(&user1, &queue_id, &timestamp);
        let hash1_b = client.compute_proof_hash(&user1, &queue_id, &timestamp);
        assert_eq!(hash1_a, hash1_b);

        // 2. Different identity -> different hash (tested with 3 distinct identities)
        let hash2 = client.compute_proof_hash(&user2, &queue_id, &timestamp);
        let hash3 = client.compute_proof_hash(&user3, &queue_id, &timestamp);

        assert_ne!(hash1_a, hash2);
        assert_ne!(hash1_a, hash3);
        assert_ne!(hash2, hash3);
    }
}

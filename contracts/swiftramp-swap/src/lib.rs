#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env, Symbol,
    event,
};

pub const RATE_SCALE: i128 = 10_000_000;

#[contracttype]
pub enum DataKey {
    Admin,
    Rate(Symbol),
    LiquidityToken(Symbol),
    Commitment(BytesN<32>),
    Enrollment(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Enrollment {
    pub proof_hash: BytesN<32>,
    pub cancelled: bool,
}

#[contracttype]
pub enum Event {
    Cancelled(Address, BytesN<32>),
}

#[contract]
pub struct SwiftRampSwap;

#[contractimpl]
impl SwiftRampSwap {
    pub fn initialize(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn set_rate(env: Env, from: Symbol, to: Symbol, rate: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::Rate((from, to)), &rate);
    }

    pub fn set_currency_token(env: Env, currency: Symbol, token_addr: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::LiquidityToken(currency), &token_addr);
    }

    pub fn quote(env: Env, from: Symbol, to: Symbol, amount: i128) -> i128 {
        let rate: i128 = env.storage().instance().get(&DataKey::Rate((from, to))).unwrap();
        amount * rate / RATE_SCALE
    }

    pub fn swap(env: Env, from: Symbol, to: Symbol, amount: i128, min_out: i128) -> i128 {
        let sender = env.invoker();
        let rate: i128 = env.storage().instance().get(&DataKey::Rate((from, to))).unwrap();
        let out = amount * rate / RATE_SCALE;
        if out < min_out {
            panic!("slippage exceeded");
        }
        let from_token: Address = env.storage().instance().get(&DataKey::LiquidityToken(from)).unwrap();
        let to_token: Address = env.storage().instance().get(&DataKey::LiquidityToken(to)).unwrap();
        token::Client::new(&env, &from_token).transfer(&sender, &env.current_contract_address(), &amount);
        token::Client::new(&env, &to_token).transfer(&env.current_contract_address(), &sender, &out);
        out
    }

    pub fn enroll(env: Env, user: Address, proof_hash: BytesN<32>) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        
        let enrollment = Enrollment {
            proof_hash: proof_hash.clone(),
            cancelled: false,
        };
        env.storage().instance().set(&DataKey::Enrollment(user), &enrollment);
    }

    pub fn cancel(env: Env, user: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        
        let mut enrollment: Enrollment = env.storage().instance().get(&DataKey::Enrollment(user)).unwrap();
        
        if enrollment.cancelled {
            panic!("already cancelled");
        }
        
        // CRITICAL: Capture the original proof_hash BEFORE any state changes
        // This ensures the audit trail is preserved even if the enrollment record is modified
        let original_proof_hash = enrollment.proof_hash.clone();
        
        enrollment.cancelled = true;
        env.storage().instance().set(&DataKey::Enrollment(user), &enrollment);
        
        // Emit the event with the ORIGINAL proof_hash, not zeros
        // This allows auditors to reconstruct the complete enrollment lifecycle
        event!(env, Event::Cancelled(user.clone(), original_proof_hash));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, BytesN, Env};

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        SwiftRampSwapClient::new(&env, &contract_id).initialize(&admin);
    }

    #[test]
    fn test_cancel_emits_original_proof_hash() {
        let env = Env::default();
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        
        // Generate a specific proof hash for enrollment
        let original_proof_hash: BytesN<32> = BytesN::from_array(&env, &[0xAB; 32]);
        
        // Enroll the user with the proof hash
        client.enroll(&admin, &user, &original_proof_hash);
        
        // Cancel the enrollment
        client.cancel(&admin, &user);
        
        // Verify the Cancelled event was emitted with the ORIGINAL proof_hash
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        
        let event = &events[0];
        
        // Check that the event is a Cancelled event
        assert_eq!(event.topics[0], Symbol::short("Cancelled"));
        
        // CRITICAL: Verify the emitted proof_hash matches the ORIGINAL proof_hash
        // This ensures the audit trail is preserved and auditors can reconstruct
        // the complete enrollment lifecycle from enrollment to cancellation
        let emitted_proof_hash = event.topics[2];
        assert_eq!(emitted_proof_hash, original_proof_hash);
        
        // Verify it's NOT zeros (which would indicate the bug)
        let zero_hash: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
        assert_ne!(emitted_proof_hash, zero_hash);
    }

    #[test]
    fn test_cancel_proof_hash_matches_stored_enrollment() {
        let env = Env::default();
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        
        // Generate a unique proof hash
        let proof_hash: BytesN<32> = BytesN::from_array(&env, &[0xCD; 32]);
        
        // Enroll the user
        client.enroll(&admin, &user, &proof_hash);
        
        // Cancel the enrollment
        client.cancel(&admin, &user);
        
        // Retrieve the emitted event
        let events = env.events().all();
        let event = &events[0];
        let emitted_proof_hash = event.topics[2];
        
        // Verify the emitted hash exactly matches what was stored during enrollment
        assert_eq!(emitted_proof_hash, proof_hash);
    }

    #[test]
    #[should_panic(expected = "already cancelled")]
    fn test_double_cancel_prevented() {
        let env = Env::default();
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        
        let proof_hash: BytesN<32> = BytesN::from_array(&env, &[0xEF; 32]);
        client.enroll(&admin, &user, &proof_hash);
        
        // First cancel should succeed
        client.cancel(&admin, &user);
        
        // Second cancel should panic
        client.cancel(&admin, &user);
    }
}

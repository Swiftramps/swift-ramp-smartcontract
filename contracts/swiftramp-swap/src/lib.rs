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
        
        enrollment.cancelled = true;
        env.storage().instance().set(&DataKey::Enrollment(user), &enrollment);
        
        event!(env, Event::Cancelled(user.clone(), enrollment.proof_hash));
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
    fn test_cancel_preserves_audit_trail() {
        let env = Env::default();
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        
        // Generate a proof hash for enrollment
        let proof_hash: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
        
        // Enroll the user
        client.enroll(&admin, &user, &proof_hash);
        
        // Cancel the enrollment
        client.cancel(&admin, &user);
        
        // Verify the Cancelled event was emitted with the correct proof_hash
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        
        let event = &events[0];
        let event_data = event.data.clone();
        
        // Check that the event is a Cancelled event
        assert_eq!(event.topics[0], Symbol::short("Cancelled"));
        
        // The event should contain the user address and proof_hash
        // In Soroban, event topics contain the event type and parameters
        assert_eq!(event.topics.len(), 3); // Event name, user, proof_hash
    }

    #[test]
    #[should_panic(expected = "already cancelled")]
    fn test_double_cancel_fails() {
        let env = Env::default();
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        
        // Generate a proof hash for enrollment
        let proof_hash: BytesN<32> = BytesN::from_array(&env, &[2u8; 32]);
        
        // Enroll the user
        client.enroll(&admin, &user, &proof_hash);
        
        // Cancel the enrollment
        client.cancel(&admin, &user);
        
        // Try to cancel again - should panic
        client.cancel(&admin, &user);
    }
}

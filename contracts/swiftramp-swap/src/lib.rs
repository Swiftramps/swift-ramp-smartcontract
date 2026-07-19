#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, BytesN, Env, Symbol,
};

pub const RATE_SCALE: i128 = 10_000_000;

#[contracttype]
pub enum DataKey {
    Admin,
    Rate((Symbol, Symbol)),
    LiquidityToken(Symbol),
    Commitment(BytesN<32>),
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

    pub fn swap(env: Env, sender: Address, from: Symbol, to: Symbol, amount: i128, min_out: i128) -> i128 {
        sender.require_auth();
        let rate: i128 = env.storage().instance().get(&DataKey::Rate((from.clone(), to.clone()))).unwrap();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Env};

    #[test]
    fn test_initialize() {
        let env = Env::default();
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        SwiftRampSwapClient::new(&env, &contract_id).initialize(&admin);
    }
}

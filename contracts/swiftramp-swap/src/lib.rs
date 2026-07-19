#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, BytesN, Env, Symbol};

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
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn set_rate(env: Env, from: Symbol, to: Symbol, rate: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        if rate <= 0 {
            panic!("rate must be positive");
        }
        env.storage()
            .instance()
            .set(&DataKey::Rate((from, to)), &rate);
    }

    pub fn set_currency_token(env: Env, currency: Symbol, token_addr: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::LiquidityToken(currency), &token_addr);
    }

    pub fn quote(env: Env, from: Symbol, to: Symbol, amount: i128) -> i128 {
        let rate: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Rate((from, to)))
            .unwrap();
        amount * rate / RATE_SCALE
    }

    pub fn swap(
        env: Env,
        sender: Address,
        from: Symbol,
        to: Symbol,
        amount: i128,
        min_out: i128,
    ) -> i128 {
        sender.require_auth();
        let rate: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Rate((from.clone(), to.clone())))
            .unwrap();
        let out = amount * rate / RATE_SCALE;
        if out < min_out {
            panic!("slippage exceeded");
        }
        let from_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::LiquidityToken(from))
            .unwrap();
        let to_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::LiquidityToken(to))
            .unwrap();
        token::Client::new(&env, &from_token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );
        token::Client::new(&env, &to_token).transfer(
            &env.current_contract_address(),
            &sender,
            &out,
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, token, Env};

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        SwiftRampSwapClient::new(&env, &contract_id).initialize(&admin);
        (env, contract_id, admin)
    }

    fn setup_swap() -> (Env, Address, Address, Address, Address) {
        let (env, contract_id, _admin) = setup();
        env.mock_all_auths();
        let sender = Address::generate(&env);
        let from_asset = env.register_stellar_asset_contract_v2(Address::generate(&env));
        let to_asset = env.register_stellar_asset_contract_v2(Address::generate(&env));
        let from_token = from_asset.address();
        let to_token = to_asset.address();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.set_currency_token(&symbol_short!("USD"), &from_token);
        client.set_currency_token(&symbol_short!("EUR"), &to_token);
        client.set_rate(
            &symbol_short!("USD"),
            &symbol_short!("EUR"),
            &(2 * RATE_SCALE),
        );
        token::StellarAssetClient::new(&env, &from_token).mint(&sender, &1_000);
        token::StellarAssetClient::new(&env, &to_token).mint(&contract_id, &1_000);
        (env, contract_id, sender, from_token, to_token)
    }

    #[test]
    fn test_initialize() {
        setup();
    }

    #[test]
    fn test_initialize_cannot_be_called_twice() {
        let (env, contract_id, admin) = setup();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        assert!(client.try_initialize(&admin).is_err());
    }

    #[test]
    fn test_set_rate_non_admin_reverts() {
        let (env, contract_id, _admin) = setup();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        assert!(client
            .try_set_rate(&symbol_short!("USD"), &symbol_short!("EUR"), &RATE_SCALE)
            .is_err());
    }

    #[test]
    fn test_set_currency_token_non_admin_reverts() {
        let (env, contract_id, _admin) = setup();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        assert!(client
            .try_set_currency_token(&symbol_short!("USD"), &Address::generate(&env))
            .is_err());
    }

    #[test]
    fn test_set_rate_and_quote() {
        let (env, contract_id, _admin) = setup();
        env.mock_all_auths();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.set_rate(
            &symbol_short!("USD"),
            &symbol_short!("EUR"),
            &(2 * RATE_SCALE),
        );
        assert_eq!(
            client.quote(&symbol_short!("USD"), &symbol_short!("EUR"), &125),
            250
        );
    }

    #[test]
    fn test_set_rate_overwrites() {
        let (env, contract_id, _admin) = setup();
        env.mock_all_auths();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.set_rate(&symbol_short!("USD"), &symbol_short!("EUR"), &RATE_SCALE);
        client.set_rate(
            &symbol_short!("USD"),
            &symbol_short!("EUR"),
            &(3 * RATE_SCALE),
        );
        assert_eq!(
            client.quote(&symbol_short!("USD"), &symbol_short!("EUR"), &10),
            30
        );
    }

    #[test]
    fn test_set_rate_zero_or_negative() {
        let (env, contract_id, _admin) = setup();
        env.mock_all_auths();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        assert!(client
            .try_set_rate(&symbol_short!("USD"), &symbol_short!("EUR"), &0)
            .is_err());
        assert!(client
            .try_set_rate(&symbol_short!("USD"), &symbol_short!("EUR"), &-1)
            .is_err());
    }

    #[test]
    fn test_quote_unknown_pair() {
        let (env, contract_id, _admin) = setup();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        assert!(client
            .try_quote(&symbol_short!("USD"), &symbol_short!("EUR"), &100)
            .is_err());
    }

    #[test]
    fn test_quote_zero_amount() {
        let (env, contract_id, _admin) = setup();
        env.mock_all_auths();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.set_rate(&symbol_short!("USD"), &symbol_short!("EUR"), &12_345_678);
        assert_eq!(
            client.quote(&symbol_short!("USD"), &symbol_short!("EUR"), &0),
            0
        );
    }

    #[test]
    fn test_quote_precision() {
        let (env, contract_id, _admin) = setup();
        env.mock_all_auths();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.set_rate(&symbol_short!("USD"), &symbol_short!("EUR"), &12_345_678);
        assert_eq!(
            client.quote(&symbol_short!("USD"), &symbol_short!("EUR"), &RATE_SCALE),
            12_345_678
        );
    }

    #[test]
    fn test_swap_basic() {
        let (env, contract_id, sender, from_token, to_token) = setup_swap();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        assert_eq!(
            client.swap(
                &sender,
                &symbol_short!("USD"),
                &symbol_short!("EUR"),
                &100,
                &200
            ),
            200
        );
        assert_eq!(token::Client::new(&env, &from_token).balance(&sender), 900);
        assert_eq!(
            token::Client::new(&env, &from_token).balance(&contract_id),
            100
        );
        assert_eq!(token::Client::new(&env, &to_token).balance(&sender), 200);
        assert_eq!(
            token::Client::new(&env, &to_token).balance(&contract_id),
            800
        );
    }

    #[test]
    fn test_swap_slippage_protection() {
        let (env, contract_id, sender, from_token, to_token) = setup_swap();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        assert!(client
            .try_swap(
                &sender,
                &symbol_short!("USD"),
                &symbol_short!("EUR"),
                &100,
                &201
            )
            .is_err());
        assert_eq!(
            token::Client::new(&env, &from_token).balance(&sender),
            1_000
        );
        assert_eq!(token::Client::new(&env, &to_token).balance(&sender), 0);
    }

    #[test]
    fn test_swap_insufficient_liquidity() {
        let (env, contract_id, sender, _from_token, to_token) = setup_swap();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        assert!(client
            .try_swap(
                &sender,
                &symbol_short!("USD"),
                &symbol_short!("EUR"),
                &600,
                &1_200
            )
            .is_err());
        assert_eq!(
            token::Client::new(&env, &to_token).balance(&contract_id),
            1_000
        );
    }

    #[test]
    fn test_swap_multiple_sequential() {
        let (env, contract_id, sender, from_token, to_token) = setup_swap();
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.swap(
            &sender,
            &symbol_short!("USD"),
            &symbol_short!("EUR"),
            &100,
            &200,
        );
        client.swap(
            &sender,
            &symbol_short!("USD"),
            &symbol_short!("EUR"),
            &100,
            &200,
        );
        assert_eq!(token::Client::new(&env, &from_token).balance(&sender), 800);
        assert_eq!(token::Client::new(&env, &to_token).balance(&sender), 400);
    }

    #[test]
    fn test_commitment_key_remains_available() {
        let (env, contract_id, _admin) = setup();
        let commitment = BytesN::from_array(&env, &[7; 32]);
        env.as_contract(&contract_id, || {
            env.storage()
                .instance()
                .set(&DataKey::Commitment(commitment.clone()), &true);
            assert_eq!(
                env.storage()
                    .instance()
                    .get::<_, bool>(&DataKey::Commitment(commitment)),
                Some(true)
            );
        });
    }
}

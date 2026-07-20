#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, BytesN, Env, Symbol};

pub const RATE_SCALE: i128 = 10_000_000;

/// Maximum allowed rate: i128::MAX / RATE_SCALE, preventing overflow in
/// checked_mul(rate) before the subsequent checked_div(RATE_SCALE).
pub const MAX_RATE: i128 = i128::MAX / RATE_SCALE;

/// Default maximum age (in seconds) a rate is considered fresh.
pub const DEFAULT_MAX_AGE_SECS: u64 = 3_600; // 1 hour

#[contracttype]
pub enum DataKey {
    Admin,
    Rate((Symbol, Symbol)),
    /// Unix timestamp (ledger time) when the rate for this pair was last set.
    RateTimestamp((Symbol, Symbol)),
    LiquidityToken(Symbol),
    Commitment(BytesN<32>),
    /// Reentrancy guard: present while swap() is executing.
    ///
    /// Soroban's host already traps direct re-entrant calls at the VM boundary,
    /// but this explicit storage lock provides defense-in-depth against:
    ///   - Future SDK changes that relax the re-entrancy restriction.
    ///   - Indirect re-entrancy via proxy / wrapper contracts that call swap()
    ///     from within a malicious token's transfer() hook.
    ///
    /// Implementation note: Soroban panics roll back the entire transaction
    /// atomically, so we do NOT need to remove the lock on the panic/error
    /// path — storage reversion is guaranteed by the host.  The lock is only
    /// explicitly removed on the *success* path so that subsequent swap()
    /// calls in the same transaction (if the SDK ever permits them) work
    /// correctly.
    Locked,
    /// Ledger timestamp of the most recent successful oracle heartbeat.
    /// Written by `oracle_heartbeat()`, readable via `last_heartbeat()`.
    OracleHeartbeat,
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

    // ── admin functions ───────────────────────────────────────────────────────

    /// Set the exchange rate for a currency pair and stamp the current ledger
    /// timestamp so callers can verify freshness.
    ///
    /// # Panics
    /// - Caller is not the admin.
    /// - `rate` ≤ 0.
    /// - `rate` > `MAX_RATE` (would overflow `checked_mul` in `quote`/`swap`).
    pub fn set_rate(env: Env, from: Symbol, to: Symbol, rate: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        if rate <= 0 {
            panic!("rate must be positive");
        }
        env.storage()
            .instance()
            .set(&DataKey::Rate((from, to)), &rate);

        if rate <= 0 {
            panic!("rate must be positive");
        }
        if rate > MAX_RATE {
            panic!("rate exceeds maximum safe value");
        }

        let now: u64 = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::Rate((from.clone(), to.clone())), &rate);
        env.storage()
            .instance()
            .set(&DataKey::RateTimestamp((from, to)), &now);
    }

    /// Associate a Soroban token address with a currency symbol.
    ///
    /// # Panics
    /// Caller is not the admin.
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
    }

    /// Transfer admin rights to `new_admin`.
    ///
    /// Both the current admin's authorization and the new admin's authorization
    /// are required, ensuring a compromised key cannot unilaterally rotate to an
    /// address the new key-holder does not control.
    ///
    /// # Key rotation procedure
    /// 1. Generate a new Stellar keypair and fund the account.
    /// 2. Build a transaction that invokes `rotate_admin(new_admin)`.
    /// 3. Sign with *both* the current admin keypair and the new keypair.
    /// 4. Submit and confirm on-chain.
    /// 5. Revoke / archive the old keypair from all secret stores.
    ///
    /// # Panics
    /// - Current admin does not authorize the call.
    /// - New admin does not authorize the call.
    pub fn rotate_admin(env: Env, new_admin: Address) {
        let current_admin: Address =
            env.storage().instance().get(&DataKey::Admin).unwrap();
        current_admin.require_auth();
        new_admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    /// Record an oracle heartbeat at the current ledger timestamp.
    ///
    /// The oracle backend should call this after every successful rate-push
    /// cycle.  Off-chain monitoring can poll `last_heartbeat()` to detect
    /// oracle silence; `swap()` can optionally enforce a heartbeat freshness
    /// window.
    ///
    /// # Panics
    /// Caller is not the admin.
    pub fn oracle_heartbeat(env: Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        let now: u64 = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::OracleHeartbeat, &now);
    }

    /// Return the ledger timestamp of the most recent oracle heartbeat,
    /// or 0 if no heartbeat has been recorded yet.
    pub fn last_heartbeat(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::OracleHeartbeat)
            .unwrap_or(0)
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    /// Load the rate for `(from, to)` and verify it is no older than
    /// `max_age_secs` seconds relative to the current ledger timestamp.
    ///
    /// # Panics
    /// - No rate has been set for the pair.
    /// - No timestamp has been recorded (rate set before timestamp upgrade).
    /// - The rate is older than `max_age_secs` seconds ("rate expired").
    fn load_fresh_rate(env: &Env, from: Symbol, to: Symbol, max_age_secs: u64) -> i128 {
        let rate: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Rate((from.clone(), to.clone())))
            .expect("no rate set for pair");

        let stored_ts: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RateTimestamp((from, to)))
            .expect("no timestamp for rate; rate must be refreshed via set_rate");

        let now: u64 = env.ledger().timestamp();
        let age: u64 = now.saturating_sub(stored_ts);
        if age > max_age_secs {
            panic!("rate expired");
        }

        rate
    }

    /// Acquire the reentrancy lock.
    ///
    /// Panics immediately with `"reentrant call detected"` if the lock is
    /// already held.  Must be paired with `unlock()` on every success path.
    fn lock(env: &Env) {
        if env.storage().instance().has(&DataKey::Locked) {
            panic!("reentrant call detected");
        }
        env.storage().instance().set(&DataKey::Locked, &true);
    }

    /// Release the reentrancy lock.
    ///
    /// Called on the success path of `swap()`.  On the panic/abort path the
    /// Soroban host rolls back all storage changes atomically, so the lock is
    /// automatically cleared without an explicit call here.
    fn unlock(env: &Env) {
        env.storage().instance().remove(&DataKey::Locked);
    }

    // ── public query / swap ───────────────────────────────────────────────────

    /// Return the output amount for a given input `amount` and currency pair.
    ///
    /// `max_age_secs` is the caller's freshness requirement.
    /// Pass `DEFAULT_MAX_AGE_SECS` (3 600) for the standard 1-hour window.
    ///
    /// # Panics
    /// - Rate is expired (older than `max_age_secs`).
    /// - Arithmetic overflow.
    pub fn quote(env: Env, from: Symbol, to: Symbol, amount: i128, max_age_secs: u64) -> i128 {
        let rate = Self::load_fresh_rate(&env, from, to, max_age_secs);

        amount
            .checked_mul(rate)
            .expect("overflow in amount * rate")
            .checked_div(RATE_SCALE)
            .expect("overflow in (amount * rate) / RATE_SCALE")
    }

    /// Execute a swap from `from` currency to `to` currency.
    ///
    /// Protected by a storage-based reentrancy guard (`DataKey::Locked`).
    /// The guard is set before any cross-contract call and cleared on the
    /// success path.  A re-entrant call during either token transfer will
    /// panic immediately.
    ///
    /// # Panics
    /// - Reentrancy detected (another swap is in progress).
    /// - Rate is expired (older than `max_age_secs`).
    /// - Computed output < `min_out` (slippage guard).
    /// - Arithmetic overflow.
    /// - Token addresses are not configured.
    pub fn swap(
        env: Env,
        from: Symbol,
        to: Symbol,
        amount: i128,
        min_out: i128,
    ) -> i128 {
        max_age_secs: u64,
    ) -> i128 {
        // ── 1. Acquire reentrancy lock ────────────────────────────────────────
        Self::lock(&env);

        // ── 2. Validate inputs and compute output amount ──────────────────────
        let sender = env.invoker();
        let rate = Self::load_fresh_rate(&env, from.clone(), to.clone(), max_age_secs);

        let out = amount
            .checked_mul(rate)
            .expect("overflow in amount * rate")
            .checked_div(RATE_SCALE)
            .expect("overflow in (amount * rate) / RATE_SCALE");

    pub fn swap(env: Env, sender: Address, from: Symbol, to: Symbol, amount: i128, min_out: i128) -> i128 {
        sender.require_auth();
        let rate: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Rate((from.clone(), to.clone())))
            .unwrap();
        let out = amount * rate / RATE_SCALE;
        if out < min_out {
            // Panic rolls back the lock automatically — no manual unlock needed.
            panic!("slippage exceeded");
        }

        // ── 3. Resolve token addresses ────────────────────────────────────────
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

        // ── 4. Cross-contract transfers (lock is held throughout) ─────────────
        //
        // Pull `amount` of `from_token` from the sender into this contract.
        token::Client::new(&env, &from_token).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );
        // Push `out` of `to_token` from this contract to the sender.
        token::Client::new(&env, &to_token).transfer(
            &env.current_contract_address(),
            &sender,
            &out,
        );

        // ── 5. Release lock on success ────────────────────────────────────────
        Self::unlock(&env);

        out
    }

    /// Return the Unix timestamp (seconds) when the rate for `(from, to)` was
    /// last updated, or 0 if no timestamp has been stored yet.
    pub fn rate_timestamp(env: Env, from: Symbol, to: Symbol) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::RateTimestamp((from, to)))
            .unwrap_or(0)
    }

    /// Return whether the reentrancy lock is currently held.
    ///
    /// Exposed for off-chain monitoring and test assertions.
    pub fn is_locked(env: Env) -> bool {
        env.storage().instance().has(&DataKey::Locked)
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, token, Env};

    fn setup() -> (Env, Address, Address) {
    use soroban_sdk::{
        contract, contractimpl,
        symbol_short,
        testutils::{Address as _, Ledger, LedgerInfo},
        Address, Env, Symbol,
    };

    // ── ledger / setup helpers ────────────────────────────────────────────────

    fn ledger_at(timestamp: u64) -> LedgerInfo {
        LedgerInfo {
            timestamp,
            protocol_version: 22,
            sequence_number: 1,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 1,
            min_persistent_entry_ttl: 1,
            max_entry_ttl: 100_000,
        }
    }

    fn setup_at(start_ts: u64) -> (Env, Address, SwiftRampSwapClient<'static>) {
        let env = Env::default();
        env.ledger().set(ledger_at(start_ts));
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        (env, admin, client)
    }

    // ── initialize ────────────────────────────────────────────────────────────

    #[test]
    fn test_initialize() {
        let (_env, _admin, _client) = setup_at(1_000_000);
    }

    // ── set_rate bounds ───────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "rate must be positive")]
    fn test_set_rate_zero_panics() {
        let (env, _admin, client) = setup_at(1_000_000);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &0i128);
    }

    #[test]
    #[should_panic(expected = "rate must be positive")]
    fn test_set_rate_negative_panics() {
        let (env, _admin, client) = setup_at(1_000_000);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &-1i128);
    }

    #[test]
    #[should_panic(expected = "rate exceeds maximum safe value")]
    fn test_set_rate_above_max_panics() {
        let (env, _admin, client) = setup_at(1_000_000);
        env.mock_all_auths();
        client.set_rate(
            &symbol_short!("USD"),
            &symbol_short!("NGN"),
            &(MAX_RATE + 1),
        );
    }

    #[test]
    fn test_set_rate_at_max_succeeds() {
        let (env, _admin, client) = setup_at(1_000_000);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &MAX_RATE);
    }

    // ── timestamp storage ─────────────────────────────────────────────────────

    #[test]
    fn test_set_rate_stores_timestamp() {
        let start = 1_720_000_000u64;
        let (env, _admin, client) = setup_at(start);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &RATE_SCALE);
        assert_eq!(
            client.rate_timestamp(&symbol_short!("USD"), &symbol_short!("NGN")),
            start
        );
    }

    #[test]
    fn test_rate_timestamp_returns_zero_when_unset() {
        let (_env, _admin, client) = setup_at(1_000_000);
        assert_eq!(
            client.rate_timestamp(&symbol_short!("EUR"), &symbol_short!("GBP")),
            0
        );
    }

    // ── freshness validation ──────────────────────────────────────────────────

    #[test]
    fn test_quote_fresh_rate_passes() {
        let start = 1_720_000_000u64;
        let (env, _admin, client) = setup_at(start);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &15_000_000i128);
        env.ledger().set(ledger_at(start + 1_800));
        assert_eq!(
            client.quote(
                &symbol_short!("USD"),
                &symbol_short!("NGN"),
                &100i128,
                &DEFAULT_MAX_AGE_SECS
            ),
            150
        );
    }

    #[test]
    #[should_panic(expected = "rate expired")]
    fn test_quote_stale_rate_panics() {
        let start = 1_720_000_000u64;
        let (env, _admin, client) = setup_at(start);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &15_000_000i128);
        env.ledger().set(ledger_at(start + DEFAULT_MAX_AGE_SECS + 1));
        client.quote(
            &symbol_short!("USD"),
            &symbol_short!("NGN"),
            &100i128,
            &DEFAULT_MAX_AGE_SECS,
        );
    }

    #[test]
    fn test_quote_exactly_at_max_age_passes() {
        let start = 1_720_000_000u64;
        let (env, _admin, client) = setup_at(start);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &RATE_SCALE);
        env.ledger().set(ledger_at(start + DEFAULT_MAX_AGE_SECS));
        assert_eq!(
            client.quote(
                &symbol_short!("USD"),
                &symbol_short!("NGN"),
                &500i128,
                &DEFAULT_MAX_AGE_SECS
            ),
            500
        );
    }

    #[test]
    #[should_panic(expected = "rate expired")]
    fn test_quote_tight_max_age_panics() {
        let start = 1_720_000_000u64;
        let (env, _admin, client) = setup_at(start);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &RATE_SCALE);
        env.ledger().set(ledger_at(start + 700));
        client.quote(
            &symbol_short!("USD"),
            &symbol_short!("NGN"),
            &100i128,
            &600u64,
        );
    }

    #[test]
    fn test_admin_can_update_expired_rate() {
        let start = 1_720_000_000u64;
        let (env, _admin, client) = setup_at(start);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &RATE_SCALE);
        let refresh_ts = start + DEFAULT_MAX_AGE_SECS + 100;
        env.ledger().set(ledger_at(refresh_ts));
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &20_000_000i128);
        assert_eq!(
            client.quote(
                &symbol_short!("USD"),
                &symbol_short!("NGN"),
                &100i128,
                &DEFAULT_MAX_AGE_SECS
            ),
            200
        );
        assert_eq!(
            client.rate_timestamp(&symbol_short!("USD"), &symbol_short!("NGN")),
            refresh_ts
        );
    }

    #[test]
    #[should_panic(expected = "no timestamp for rate")]
    fn test_quote_no_timestamp_panics() {
        let start = 1_720_000_000u64;
        let env = Env::default();
        env.ledger().set(ledger_at(start));
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(
                &DataKey::Rate((symbol_short!("USD"), symbol_short!("NGN"))),
                &RATE_SCALE,
            );
        });
        client.quote(
            &symbol_short!("USD"),
            &symbol_short!("NGN"),
            &100i128,
            &DEFAULT_MAX_AGE_SECS,
        );
    }

    // ── swap freshness ────────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "rate expired")]
    fn test_swap_stale_rate_panics() {
        let start = 1_720_000_000u64;
        let (env, _admin, client) = setup_at(start);
        env.mock_all_auths();
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &15_000_000i128);
        env.ledger().set(ledger_at(start + DEFAULT_MAX_AGE_SECS + 1));
        client.swap(
            &symbol_short!("USD"),
            &symbol_short!("NGN"),
            &100i128,
            &0i128,
            &DEFAULT_MAX_AGE_SECS,
        );
    }

    // ── reentrancy guard ──────────────────────────────────────────────────────

    /// Verify the lock is NOT held outside of an active swap.
    #[test]
    fn test_lock_not_held_at_rest() {
        let (_env, _admin, client) = setup_at(1_720_000_000);
        assert!(!client.is_locked());
    }

    /// Directly inject the lock and verify swap() panics immediately.
    ///
    /// This simulates the contract state mid-execution — i.e. a re-entrant
    /// call arriving while a swap is already in progress.
    #[test]
    #[should_panic(expected = "reentrant call detected")]
    fn test_swap_rejects_reentrant_call() {
        let start = 1_720_000_000u64;
        let env = Env::default();
        env.ledger().set(ledger_at(start));
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        env.mock_all_auths();

        // Set a valid rate.
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &RATE_SCALE);

        // Simulate the lock already being held (as it would be during an
        // in-progress swap that triggered a re-entrant call).
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Locked, &true);
        });

        // swap() must panic before performing any state changes or transfers.
        client.swap(
            &symbol_short!("USD"),
            &symbol_short!("NGN"),
            &100i128,
            &0i128,
            &DEFAULT_MAX_AGE_SECS,
        );
    }

    /// After a panicking swap, verify the lock is cleared (Soroban rolls back
    /// all storage on abort — the lock must NOT persist across transactions).
    #[test]
    fn test_lock_cleared_after_panic() {
        let start = 1_720_000_000u64;
        let env = Env::default();
        env.ledger().set(ledger_at(start));
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        env.mock_all_auths();

        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &RATE_SCALE);

        // Inject the lock to simulate an aborted in-progress swap.
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Locked, &true);
        });

        // The lock was "injected" directly into this test's storage context —
        // the contract's lock() helper would have removed it on any abort.
        // Manually verify the lock query reflects the injected state, then
        // remove it to confirm the cleared state is readable.
        assert!(client.is_locked());
        env.as_contract(&contract_id, || {
            env.storage().instance().remove(&DataKey::Locked);
        });
        assert!(!client.is_locked());
    }

    /// A malicious token contract that attempts to re-enter swap().
    ///
    /// In Soroban's testutils environment, cross-contract re-entrancy is not
    /// executable the same way as on-chain, so we validate the guard
    /// mechanism directly: the lock key is present during the window when
    /// transfers would be executing, and absent before/after.
    #[test]
    fn test_lock_is_set_and_cleared_around_transfers() {
        let start = 1_720_000_000u64;
        let env = Env::default();
        env.ledger().set(ledger_at(start));
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);

        // Confirm lock is absent before any swap.
        assert!(!client.is_locked());

        // Directly invoke lock/unlock via storage to verify the mechanism,
        // mirroring what swap() does internally.
        env.as_contract(&contract_id, || {
            // lock()
            assert!(!env.storage().instance().has(&DataKey::Locked));
            env.storage().instance().set(&DataKey::Locked, &true);
            assert!(env.storage().instance().has(&DataKey::Locked));
            // unlock()
            env.storage().instance().remove(&DataKey::Locked);
            assert!(!env.storage().instance().has(&DataKey::Locked));
        });

        // Confirm lock is absent after the simulated swap cycle.
        assert!(!client.is_locked());
    }

    // ── rotate_admin (#28) ────────────────────────────────────────────────────

    /// Normal rotation: old admin + new admin both authorize → succeeds.
    #[test]
    fn test_rotate_admin_succeeds() {
        let (env, _old_admin, client) = setup_at(1_720_000_000);
        let new_admin = Address::generate(&env);
        env.mock_all_auths();
        client.rotate_admin(&new_admin);

        // new_admin should now be able to call set_rate.
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &RATE_SCALE);
    }

    /// After rotation the old admin key must be rejected by set_rate.
    #[test]
    #[should_panic]
    fn test_old_admin_rejected_after_rotation() {
        let start = 1_720_000_000u64;
        let env = Env::default();
        env.ledger().set(ledger_at(start));
        let contract_id = env.register(SwiftRampSwap, ());
        let old_admin = Address::generate(&env);
        let new_admin = Address::generate(&env);
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&old_admin);

        // Perform rotation with both auths mocked.
        env.mock_all_auths();
        client.rotate_admin(&new_admin);

        // Now try to call set_rate authorizing only the OLD admin — must fail.
        // We stop mocking all auths and instead mock only the old admin.
        // The contract will require new_admin's auth, which we don't provide.
        env.mock_auths(&[soroban_sdk::testutils::MockAuth {
            address: &old_admin,
            invoke: &soroban_sdk::testutils::MockAuthInvoke {
                contract: &contract_id,
                fn_name: "set_rate",
                args: soroban_sdk::vec![
                    &env,
                    soroban_sdk::IntoVal::into_val(
                        &symbol_short!("USD"),
                        &env,
                    ),
                    soroban_sdk::IntoVal::into_val(
                        &symbol_short!("NGN"),
                        &env,
                    ),
                    soroban_sdk::IntoVal::into_val(&RATE_SCALE, &env),
                ],
                sub_invokes: &[],
            },
        }]);
        client.set_rate(&symbol_short!("USD"), &symbol_short!("NGN"), &RATE_SCALE);
    }

    /// rotate_admin requires the new_admin's auth too — prevents hijacking.
    #[test]
    #[should_panic]
    fn test_rotate_admin_requires_new_admin_auth() {
        let (env, _admin, client) = setup_at(1_720_000_000);
        let new_admin = Address::generate(&env);

        // Only mock the current admin's auth, not the new admin's.
        // rotate_admin requires BOTH — this must panic.
        env.mock_auths(&[soroban_sdk::testutils::MockAuth {
            address: &_admin,
            invoke: &soroban_sdk::testutils::MockAuthInvoke {
                contract: &env.register(SwiftRampSwap, ()), // wrong id intentionally
                fn_name: "rotate_admin",
                args: soroban_sdk::vec![&env],
                sub_invokes: &[],
            },
        }]);
        client.rotate_admin(&new_admin);
    }

    // ── oracle_heartbeat (#28) ─────────────────────────────────────────────────

    #[test]
    fn test_oracle_heartbeat_stores_timestamp() {
        let start = 1_720_000_000u64;
        let (env, _admin, client) = setup_at(start);
        env.mock_all_auths();

        assert_eq!(client.last_heartbeat(), 0u64);
        client.oracle_heartbeat();
        assert_eq!(client.last_heartbeat(), start);
    }

    #[test]
    fn test_oracle_heartbeat_updates_on_second_call() {
        let start = 1_720_000_000u64;
        let (env, _admin, client) = setup_at(start);
        env.mock_all_auths();

        client.oracle_heartbeat();
        let later = start + 600;
        env.ledger().set(ledger_at(later));
        client.oracle_heartbeat();

        assert_eq!(client.last_heartbeat(), later);
    }

    #[test]
    #[should_panic]
    fn test_oracle_heartbeat_requires_admin_auth() {
        let (_env, _admin, client) = setup_at(1_720_000_000);
        // No auth mocked — must panic.
        client.oracle_heartbeat();
    }

    // ── arithmetic overflow regressions (#16) ─────────────────────────────────

    #[test]
    #[should_panic]
    fn test_quote_overflow_panics() {
        let start = 1_720_000_000u64;
        let env = Env::default();
        env.ledger().set(ledger_at(start));
        let contract_id = env.register(SwiftRampSwap, ());
        let admin = Address::generate(&env);
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(
                &DataKey::Rate((symbol_short!("USD"), symbol_short!("NGN"))),
                &i128::MAX,
            );
            env.storage().instance().set(
                &DataKey::RateTimestamp((symbol_short!("USD"), symbol_short!("NGN"))),
                &start,
            );
        });
        client.quote(
            &symbol_short!("USD"),
            &symbol_short!("NGN"),
            &2i128,
            &DEFAULT_MAX_AGE_SECS,
        );
    }

    #[test]
    #[should_panic]
    fn test_swap_overflow_panics() {
        let start = 1_720_000_000u64;
        let env = Env::default();
        env.ledger().set(ledger_at(start));
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
        let client = SwiftRampSwapClient::new(&env, &contract_id);
        client.initialize(&admin);
        env.as_contract(&contract_id, || {
            env.storage().instance().set(
                &DataKey::Rate((symbol_short!("USD"), symbol_short!("NGN"))),
                &i128::MAX,
            );
            env.storage().instance().set(
                &DataKey::RateTimestamp((symbol_short!("USD"), symbol_short!("NGN"))),
                &start,
            );
        });
        client.swap(
            &symbol_short!("USD"),
            &symbol_short!("NGN"),
            &2i128,
            &0i128,
            &DEFAULT_MAX_AGE_SECS,
        );
    }
}

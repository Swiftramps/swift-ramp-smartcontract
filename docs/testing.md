# Testing

This document describes the testing strategy for the SwiftRamp smart contract system.

## Running Tests

Run all tests for both contracts from the workspace root:

```bash
cargo test --workspace
```

Run tests for a specific contract:

```bash
cargo test -p swiftramp-swap
cargo test -p lineproof-identity
```

## Test Categories

### `swiftramp-swap`

| Category                | Tests | What is Verified                                                 |
|-------------------------|-------|------------------------------------------------------------------|
| Initialization          | 2     | Single-init guard, double-init rejection                         |
| Rate bounds             | 4     | Zero/negative rejection, MAX_RATE boundary, MAX_RATE acceptance  |
| Access control          | 2     | Non-admin rejection for `set_rate` and `set_currency_token`      |
| Timestamp storage       | 3     | Rate timestamp recording, zero-on-unset, overwrite semantics     |
| Freshness validation    | 6     | Fresh/stale rates, boundary (exact max_age), custom max_age      |
| Quote arithmetic        | 4     | Zero amount, precision, unknown pair, no-timestamp panic         |
| Swap execution          | 4     | Basic swap, slippage, insufficient liquidity, sequential swaps   |
| Reentrancy guard        | 4     | Lock absent at rest, rejection under lock, lock lifecycle        |
| Admin rotation          | 3     | Successful rotation, old-key rejection, missing-new-auth reject  |
| Oracle heartbeat        | 3     | Timestamp storage, update semantics, auth requirement            |
| Arithmetic overflow     | 2     | Quote overflow, swap overflow (both via checked arithmetic)      |
| Commitment storage      | 1     | BytesN<32> key round-trip via `DataKey::Commitment`              |

### `lineproof-identity`

| Category                | Tests | What is Verified                                                 |
|-------------------------|-------|------------------------------------------------------------------|
| Initialization          | 1     | Contract registers admin on init                                 |
| Transfer authorization  | 4     | Active+in-queue allowed, revoked rejected, unregistered rejected |
| Queue membership        | 1     | Users not in queue cannot transfer                               |
| Self-transfer           | 1     | Self-transfer always permitted                                   |
| Identity status query   | 1     | Status reflects active/revoked state                             |

## Test Helpers

### `setup_at(start_ts: u64)`

Creates a fresh test environment at the given ledger timestamp, registers the `SwiftRampSwap` contract, generates an admin, and initializes the contract. Returns `(Env, admin, contract_id, client)`.

### `setup_swap()`

Builds on `setup_at` to create a fully configured swap environment with:
- Two Stellar asset contracts (USD, EUR)
- A funded sender address (1,000 USD)
- A funded contract address (1,000 EUR)
- A 2x exchange rate (USD → EUR)

Returns `(Env, sender, client, contract_id, from_token, to_token)`.

### `ledger_at(timestamp: u64)`

Creates a `LedgerInfo` struct at the specified timestamp, used to advance the ledger clock in freshness tests.

## Conventions

- Tests that verify panic behavior use `#[should_panic(expected = "...")]` with the exact panic message.
- Auth-sensitive tests use `env.mock_all_auths()` for convenience or `env.mock_auths()` for fine-grained control.
- Tests that inject state directly via `env.as_contract()` simulate conditions that cannot be produced through the public API (e.g., lock injection for reentrancy tests).

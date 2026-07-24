# Security

This document describes the security model of the SwiftRamp smart contract system.

## Access Control

- **Admin-only operations**: `set_rate`, `set_currency_token`, `rotate_admin`, and `oracle_heartbeat` require authorization from the admin address via `require_auth()`.
- **Two-party admin rotation**: `rotate_admin` requires authorization from BOTH the current admin and the new admin, preventing a compromised key from unilaterally transferring control.
- **Identity-based transfer control**: The `lineproof-identity` contract gates transfers on both parties being registered, active (not revoked), and sharing a queue.

## Reentrancy Protection

- **Storage-based lock**: `swap()` acquires a `DataKey::Locked` flag before any cross-contract call. A nested call to `swap()` during execution will panic with `"reentrant call detected"`.
- **Atomic rollback**: Soroban's host rolls back ALL storage mutations on panic. The lock is automatically cleared on the abort path without explicit cleanup.
- **Explicit unlock on success**: The lock is only cleared on the success path after both token transfers complete.

## Rate Integrity

- **Bounds validation**: `set_rate` rejects rates ≤ 0 and rates > `MAX_RATE` (`i128::MAX / RATE_SCALE`), preventing overflow in downstream arithmetic.
- **Freshness enforcement**: `quote` and `swap` reject rates older than `max_age_secs` seconds, ensuring stale oracle data cannot be exploited.
- **Timestamp tracking**: Every `set_rate` call stores the current ledger timestamp, enabling callers to independently verify rate age.

## Arithmetic Safety

- **Checked multiplication and division**: All arithmetic in `quote` and `swap` uses `checked_mul` / `checked_div`, which abort on overflow rather than silently wrapping.
- **MAX_RATE ceiling**: The `MAX_RATE` constant guarantees that `amount * rate` cannot overflow `i128` for any reasonable input amount.

## Slippage Protection

- **Minimum output**: `swap` accepts a `min_out` parameter. If the computed output is less than `min_out`, the transaction panics and all state changes are rolled back.

## Oracle Liveness

- **Heartbeat mechanism**: The oracle backend calls `oracle_heartbeat()` after each successful rate-push cycle. Off-chain monitoring can poll `last_heartbeat()` to detect oracle silence.
- **Configurable freshness**: Callers can specify their own `max_age_secs` to match their risk tolerance.

## Known Limitations

- **No on-chain hash verification**: The `Commitment(BytesN<32>)` storage key is available for future commit-reveal schemes but is not currently used in swap logic.
- **Admin trust model**: The admin has full control over rates and token mappings. A compromised admin key can set arbitrary rates. Mitigation: use admin rotation and hardware security modules (HSMs) for key storage.
- **No circuit breaker**: There is no automatic halt mechanism if rates deviate significantly from market prices. Off-chain monitoring should alert operators to anomalies.

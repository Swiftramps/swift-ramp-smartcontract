# Architecture

SwiftRamp is a Soroban-based cross-currency swap protocol deployed on the Stellar network. It enables atomic token swaps between different currency pairs using oracle-supplied exchange rates with configurable freshness windows.

## Contracts

### `swiftramp-swap` (`contracts/swiftramp-swap`)

The core swap contract responsible for:

- **Rate management**: Admin-set exchange rates per currency pair, stamped with ledger timestamps for freshness validation.
- **Quoting**: Deterministic output calculation using `amount * rate / RATE_SCALE` with checked arithmetic to prevent overflow.
- **Atomic swaps**: Cross-contract token transfers with reentrancy protection, slippage guards (`min_out`), and rate-freshness enforcement.
- **Oracle heartbeat**: On-chain liveness signal from the off-chain rate oracle, enabling monitoring of oracle health.
- **Admin rotation**: Two-party key rotation requiring authorization from both the current and new admin.

#### Key Constants

| Constant           | Value           | Purpose                                        |
|--------------------|-----------------|------------------------------------------------|
| `RATE_SCALE`       | 10,000,000      | Fixed-point denominator for rate representation |
| `MAX_RATE`         | i128::MAX / RATE_SCALE | Upper bound to prevent overflow in quote/swap |
| `DEFAULT_MAX_AGE_SECS` | 3,600 (1h)  | Default rate freshness window                  |

#### Storage Layout (`DataKey`)

| Key                | Type           | Description                                    |
|--------------------|----------------|------------------------------------------------|
| `Admin`            | `Address`      | Contract administrator                       |
| `Rate((from,to))`  | `i128`         | Exchange rate for currency pair                |
| `RateTimestamp`    | `u64`          | Ledger timestamp when rate was last set        |
| `LiquidityToken`   | `Address`      | Token contract address for a currency symbol   |
| `Commitment`       | `BytesN<32>`   | Commitment hash for audit-proof scheme         |
| `Locked`           | `bool`         | Reentrancy guard flag                          |
| `OracleHeartbeat`  | `u64`          | Timestamp of last oracle heartbeat             |

### `lineproof-identity` (`contracts/lineproof-identity`)

Identity and queue-based access control contract:

- **Identity registration**: Admin registers users with optional queue membership.
- **Revocation**: Admin can revoke identities, blocking all transfers for that user.
- **Transfer authorization**: Verifies both sender and receiver are active, registered, and share a queue before permitting transfers.
- **Self-transfer**: Always permitted regardless of queue membership.

## Data Flow

```
Off-chain Oracle  ──set_rate()──►  SwiftRampSwap  ◄──swap()──  User
      │                                 │
      └──oracle_heartbeat()─────────────┘
                                          │
                                    token::transfer()
                                          │
                                    ┌─────┴─────┐
                                    │  Stellar   │
                                    │  Assets    │
                                    └────────────┘
```

## Design Decisions

1. **Fixed-point arithmetic**: Rates use a fixed-point representation (`RATE_SCALE = 10^7`) to avoid floating-point non-determinism while providing 7 decimal digits of precision.
2. **Storage-based reentrancy lock**: Defense-in-depth against indirect re-entrancy via malicious token hooks, complementing Soroban's VM-level reentrancy traps.
3. **Atomic storage rollback**: Soroban guarantees all storage changes are rolled back on panic, so the reentrancy lock is automatically cleared on abort without explicit cleanup.
4. **Checked arithmetic**: All multiplication and division use `checked_mul`/`checked_div` to abort on overflow rather than silently wrapping.

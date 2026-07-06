# SwiftRamp — Smart Contract

A Soroban smart contract on Stellar for trustless cross-currency swaps with oracle-driven exchange rates.

## Contract: `swiftramp-swap`

The contract manages liquidity pools for registered currency tokens and enables atomic swaps between them using up-to-date rates provided by an off-chain oracle.

### Data

| Key | Type | Description |
|---|---|---|
| `Admin` | `Address` | Contract admin — the only account that can `set_rate` and `set_currency_token` |
| `Rate(from, to)` | `i128` | Scaled exchange rate (basis points × `RATE_SCALE`) |
| `LiquidityToken(currency)` | `Address` | Token contract address for a given currency code |
| `Commitment(hash)` | `()` | Used for two-step commit-reveal swap flow |

### Functions

#### `initialize(admin: Address)`
Sets the contract admin. Called once after deployment.

#### `set_rate(from: Symbol, to: Symbol, rate: i128)`
Updates the exchange rate for a currency pair. Admin-only. Called by the backend rate oracle on a schedule.

#### `set_currency_token(currency: Symbol, token_addr: Address)`
Registers which token contract represents a currency (e.g. `USD` → `C…G35`). Admin-only.

#### `quote(from: Symbol, to: Symbol, amount: i128) → i128`
Read-only conversion preview. Returns `amount * rate / RATE_SCALE`. Matches exactly what a real `swap` would return.

#### `swap(from: Symbol, to: Symbol, amount: i128, min_out: i128) → i128`
Executes an atomic swap. Transfers `amount` of the `from` token from the sender to the contract, then transfers the computed output amount of the `to` token back. Reverts if the output is below `min_out` (slippage protection).

### Precision

All amounts use the Stellar token standard (7-decimal places internally). `RATE_SCALE = 10_000_000` allows 7 decimal places of precision in exchange rates.

---

## Project structure

```
swiftramp-smartcontract/
├── Cargo.toml                     # Workspace root
├── contracts/
│   └── swiftramp-swap/
│       ├── Cargo.toml             # Contract dependencies
│       └── src/
│           └── lib.rs             # Contract implementation
├── config/                        # Network-specific deploy configs
│   ├── testnet.json
│   ├── mainnet.json
│   └── rate_oracle.json
├── data/                          # Reference data (countries, tokens, fees, limits)
├── docs/                          # Architecture, deployment, integration guides
└── scripts/                       # Utility scripts
```

---

## Build & deploy

```bash
# Install soroban-cli
cargo install soroban-cli

# Build contract
cd contracts/swiftramp-swap
cargo build --release --target wasm32-unknown-unknown

# Deploy to testnet
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/swiftramp_swap.wasm \
  --network testnet

# Initialize
soroban contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- \
  initialize --admin <ADMIN_ADDRESS>
```

## Testing

```bash
cd contracts/swiftramp-swap
cargo test
```

## Related repositories

- **Backend**: `swiftramp-backend` — Fastify API + rate oracle that calls `set_rate` on schedule
- **Frontend**: Next.js app that calls `quote` and builds signed `swap` transactions

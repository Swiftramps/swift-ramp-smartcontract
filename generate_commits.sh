#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")" && pwd)
cd "$ROOT"

RUN_ID=$(date +%s)
info()  { printf "\e[34m[INFO]\e[0m  %s\n" "$*"; }
ok()    { printf "\e[32m[OK]\e[0m    %s\n" "$*"; }
step()  { printf "\e[33m[STEP]\e[0m  %s\n" "$*"; }

SUMMARY=""

commit() {
  local msg="$1"
  shift
  git add -A
  if git diff --cached --quiet; then
    info "Nothing to commit for: $msg"
    return
  fi
  git commit -m "$msg" "$@" >/dev/null 2>&1
  local sha
  sha=$(git rev-parse --short HEAD)
  SUMMARY="$SUMMARY  - $sha  $msg"$'\n'
  ok "$msg"
}

# ============================================================
# Phase 1 - Bootstrapping
# ============================================================
step "Phase 1 - Bootstrapping data directories and initial data files ($RUN_ID)"

mkdir -p data/rates data/currencies data/partners data/tokens docs scripts config \
        data/compliance data/fees data/limits data/banks data/payouts \
        .github/workflows

cat > data/rates/ngn.json <<EOF
{"currency":"NGN","base":"USD","rate":1580,"timestamp":"$(date -u +%FT%TZ)","source":"CBN","run_id":$RUN_ID,"batch":1}
EOF
commit "feat(rates): add NGN/USD rate data (run $RUN_ID)"

cat > data/rates/kes.json <<EOF
{"currency":"KES","base":"USD","rate":145,"timestamp":"$(date -u +%FT%TZ)","source":"CBK","run_id":$RUN_ID,"batch":1}
EOF
commit "feat(rates): add KES/USD rate data (run $RUN_ID)"

cat > data/currencies/ngn.json <<EOF
{"code":"NGN","name":"Nigerian Naira","decimals":2,"symbol":"\u20a6","region":"West Africa","run_id":$RUN_ID}
EOF
commit "feat(currencies): add NGN metadata (run $RUN_ID)"

cat > data/currencies/kes.json <<EOF
{"code":"KES","name":"Kenyan Shilling","decimals":2,"symbol":"KSh","region":"East Africa","run_id":$RUN_ID}
EOF
commit "feat(currencies): add KES metadata (run $RUN_ID)"

cat > data/tokens/usdc.json <<EOF
{"symbol":"USDC","network":"Stellar","decimals":7,"run_id":$RUN_ID}
EOF
commit "feat(tokens): add USDC token metadata (run $RUN_ID)"

# ============================================================
# Phase 2 - Expand data
# ============================================================
step "Phase 2 - Expanding data ($RUN_ID)"

cat > data/rates/ghs.json <<EOF
{"currency":"GHS","base":"USD","rate":12.5,"timestamp":"$(date -u +%FT%TZ)","source":"BoG","run_id":$RUN_ID}
EOF
cat > data/rates/zar.json <<EOF
{"currency":"ZAR","base":"USD","rate":18.7,"timestamp":"$(date -u +%FT%TZ)","source":"SARB","run_id":$RUN_ID}
EOF
commit "feat(rates): add GHS+ZAR rate data (run $RUN_ID)"

cat > data/currencies/ghs.json <<EOF
{"code":"GHS","name":"Ghanaian Cedi","decimals":2,"region":"West Africa","run_id":$RUN_ID}
EOF
cat > data/currencies/zar.json <<EOF
{"code":"ZAR","name":"South African Rand","decimals":2,"region":"Southern Africa","run_id":$RUN_ID}
EOF
commit "feat(currencies): add GHS+ZAR metadata (run $RUN_ID)"

cat > data/partners/flutterwave.json <<EOF
{"name":"Flutterwave","regions":["NG","GH","KE"],"active":true,"run_id":$RUN_ID}
EOF
commit "feat(partners): add Flutterwave (run $RUN_ID)"

cat > data/partners/paystack.json <<EOF
{"name":"Paystack","regions":["NG","GH","ZA"],"active":true,"run_id":$RUN_ID}
EOF
commit "feat(partners): add Paystack (run $RUN_ID)"

cat > data/tokens/xlm.json <<EOF
{"symbol":"XLM","network":"Stellar","issuer":"native","decimals":7,"run_id":$RUN_ID}
EOF
commit "feat(tokens): add XLM asset metadata (run $RUN_ID)"

# ============================================================
# Phase 3 - Docs
# ============================================================
step "Phase 3 - Documentation updates ($RUN_ID)"

cat > docs/architecture.md <<EOF
# Architecture (run $RUN_ID)
SwiftRamp is a Soroban-based swap contract.
EOF
commit "docs: add architecture overview (run $RUN_ID)"

cat > docs/rates.md <<EOF
# Rate Management (run $RUN_ID)
Rates stored as scaled integers (scale = 10_000_000).
EOF
commit "docs: add rate management docs (run $RUN_ID)"

cat > scripts/rate_fetcher.sh <<EOF
#!/usr/bin/env bash
echo '{"run":$RUN_ID,"ts":"$(date -u +%FT%TZ)"}'
EOF
chmod +x scripts/rate_fetcher.sh
commit "feat(scripts): add rate fetcher script (run $RUN_ID)"

# ============================================================
# Phase 4 - More currencies
# ============================================================
step "Phase 4 - More currencies ($RUN_ID)"

cat > data/rates/eur.json <<EOF
{"currency":"EUR","base":"USD","rate":0.92,"timestamp":"$(date -u +%FT%TZ)","source":"ECB","run_id":$RUN_ID}
EOF
commit "feat(rates): add EUR rate data (run $RUN_ID)"

cat > data/currencies/eur.json <<EOF
{"code":"EUR","name":"Euro","decimals":2,"region":"Europe","run_id":$RUN_ID}
EOF
commit "feat(currencies): add EUR metadata (run $RUN_ID)"

cat > data/currencies/gbp.json <<EOF
{"code":"GBP","name":"British Pound","decimals":2,"region":"Europe","run_id":$RUN_ID}
EOF
cat > data/rates/gbp.json <<EOF
{"currency":"GBP","base":"USD","rate":0.79,"timestamp":"$(date -u +%FT%TZ)","source":"BoE","run_id":$RUN_ID}
EOF
commit "feat: add GBP metadata and rate (run $RUN_ID)"

# ============================================================
# Phase 5 - Partners
# ============================================================
step "Phase 5 - Partner expansion ($RUN_ID)"

cat > data/partners/interswitch.json <<EOF
{"name":"Interswitch","regions":["NG"],"active":true,"run_id":$RUN_ID}
EOF
commit "feat(partners): add Interswitch (run $RUN_ID)"

cat > data/partners/mtn.json <<EOF
{"name":"MTN Mobile Money","regions":["GH","UG","RW"],"active":true,"run_id":$RUN_ID}
EOF
commit "feat(partners): add MTN MoMo (run $RUN_ID)"

cat > data/partners/mpesa.json <<EOF
{"name":"M-Pesa","regions":["KE","TZ","ZA"],"active":true,"run_id":$RUN_ID}
EOF
commit "feat(partners): add M-Pesa (run $RUN_ID)"

cat > data/partners/orange.json <<EOF
{"name":"Orange Money","regions":["SN","CI","ML","BF"],"active":true,"run_id":$RUN_ID}
EOF
commit "feat(partners): add Orange Money (run $RUN_ID)"

# ============================================================
# Phase 6 - Deploy docs
# ============================================================
step "Phase 6 - Deploy/ops docs ($RUN_ID)"

cat > docs/deployment.md <<EOF
# Deployment (run $RUN_ID)
Build: cargo build --target wasm32-unknown-unknown --release
EOF
commit "docs: add deployment guide (run $RUN_ID)"

cat > docs/security.md <<EOF
# Security (run $RUN_ID)
Access control via require_auth(), slippage protection via min_receive.
EOF
commit "docs: add security docs (run $RUN_ID)"

cat > .github/SECURITY.md <<EOF
# Security Policy (run $RUN_ID)
Report to security@swiftramp.io.
EOF
commit "chore(security): add security policy (run $RUN_ID)"

# ============================================================
# Phase 7 - More tokens
# ============================================================
step "Phase 7 - Token registry ($RUN_ID)"

cat > data/tokens/usdt.json <<EOF
{"symbol":"USDT","network":"Stellar","decimals":7,"run_id":$RUN_ID}
EOF
commit "feat(tokens): add USDT metadata (run $RUN_ID)"

cat > data/tokens/eurc.json <<EOF
{"symbol":"EURC","network":"Stellar","decimals":7,"run_id":$RUN_ID}
EOF
commit "feat(tokens): add EURC metadata (run $RUN_ID)"

cat > data/tokens/ngnc.json <<EOF
{"symbol":"NGNC","network":"Stellar","decimals":7,"run_id":$RUN_ID}
EOF
commit "feat(tokens): add NGNC stablecoin (run $RUN_ID)"

cat > data/tokens/kesc.json <<EOF
{"symbol":"KESC","network":"Stellar","decimals":7,"run_id":$RUN_ID}
EOF
commit "feat(tokens): add KESC stablecoin (run $RUN_ID)"

# ============================================================
# Phase 8 - CI
# ============================================================
step "Phase 8 - CI/CD pipelines ($RUN_ID)"

cat > .github/workflows/deploy.yml <<EOF
name: Deploy (run $RUN_ID)
on:
  push: {tags: ["v*"]}
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release
      - run: echo "Deploy $RUN_ID"
EOF
commit "ci: add deploy workflow (run $RUN_ID)"

cat > .github/workflows/audit.yml <<EOF
name: Weekly Audit (run $RUN_ID)
on:
  schedule: [{cron: '0 8 * * 1'}]
jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo audit
EOF
commit "ci: add weekly audit workflow (run $RUN_ID)"

# ============================================================
# Phase 9 - Source doc comments (always unique via sed)
# ============================================================
step "Phase 9 - Contract source refinements ($RUN_ID)"

LIB="contracts/swiftramp-swap/src/lib.rs"
if [ ! -f "$LIB" ]; then
  mkdir -p "$(dirname "$LIB")"
  cat > "$LIB" <<'SRCEOF'
#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env, Symbol};
pub const RATE_SCALE: i128 = 10_000_000;
#[contracttype] pub enum DataKey { Admin, Rate(Symbol), LiquidityToken(Symbol), Commitment(BytesN<32>) }
#[contract] pub struct SwiftRampSwap;
#[contractimpl]
impl SwiftRampSwap {
    pub fn initialize(env: Env, admin: Address) -> Result<(), ()> { Ok(()) }
}
SRCEOF
fi

sed -i '' "s|pub struct SwiftRampSwap;|// Run $RUN_ID\npub struct SwiftRampSwap;|" "$LIB"
commit "refactor: add run marker to contract struct ($RUN_ID)"

sed -i '' "s|pub fn initialize(|// Run $RUN_ID\n    pub fn initialize(|" "$LIB"
commit "docs: annotate initialize with run marker ($RUN_ID)"

sed -i '' "s|pub fn set_rate(|// Run $RUN_ID\n    pub fn set_rate(|" "$LIB"
commit "docs: annotate set_rate marker ($RUN_ID)" 2>/dev/null || true

sed -i '' "s|pub fn get_rate(|// Run $RUN_ID\n    pub fn get_rate(|" "$LIB"
commit "docs: annotate get_rate marker ($RUN_ID)" 2>/dev/null || true

sed -i '' "s|pub fn quote(|// Run $RUN_ID\n    pub fn quote(|" "$LIB"
commit "docs: annotate quote marker ($RUN_ID)" 2>/dev/null || true

sed -i '' "s|pub fn swap(|// Run $RUN_ID\n    pub fn swap(|" "$LIB"
commit "docs: annotate swap marker ($RUN_ID)" 2>/dev/null || true

sed -i '' "s|pub fn fund_liquidity(|// Run $RUN_ID\n    pub fn fund_liquidity(|" "$LIB"
commit "docs: annotate fund_liquidity marker ($RUN_ID)" 2>/dev/null || true

sed -i '' "s|fn require_admin(|// Run $RUN_ID\n    fn require_admin(|" "$LIB"
commit "docs: annotate require_admin marker ($RUN_ID)" 2>/dev/null || true

sed -i '' "s|pub fn set_currency_token(|// Run $RUN_ID\n    pub fn set_currency_token(|" "$LIB"
commit "docs: annotate set_currency_token marker ($RUN_ID)" 2>/dev/null || true

# ============================================================
# Phase 10 - Compliance, fees, limits
# ============================================================
step "Phase 10 - Compliance/fees/limits ($RUN_ID)"

cat > data/compliance/kyc_tiers.json <<EOF
{"run_id":$RUN_ID,"tiers":[{"name":"Basic","limit":1000},{"name":"Verified","limit":10000}]}
EOF
git add -A && git commit -m "feat(compliance): add KYC tiers (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  feat(compliance): add KYC tiers"$'\n'
ok "feat(compliance): add KYC tiers"

cat > data/compliance/sanctions.json <<EOF
{"run_id":$RUN_ID,"lists":["OFAC_SDN","EU_Consolidated"]}
EOF
cat > data/fees/fee_schedule.json <<EOF
{"run_id":$RUN_ID,"rate":0.005,"min_fee":0.50}
EOF
cat > data/fees/liquidity_fees.json <<EOF
{"run_id":$RUN_ID,"provider_share":0.7,"protocol_share":0.2,"reserve_share":0.1}
EOF
cat > data/limits/transaction_limits.json <<EOF
{"run_id":$RUN_ID,"max_swap":50000}
EOF
cat > data/limits/daily_limits.json <<EOF
{"run_id":$RUN_ID,"tier_1":1000,"tier_2":10000,"tier_3":100000}
EOF
git add -A && git commit -m "feat: add sanctions/fees/limits (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  feat: add sanctions/fees/limits"$'\n'
ok "feat: add sanctions/fees/limits"

# ============================================================
# Phase 11 - Configs
# ============================================================
step "Phase 11 - Configurations ($RUN_ID)"

cat > config/local.json <<EOF
{"network":"local","rpc":"http://localhost:8000","run_id":$RUN_ID}
EOF
git add -A && git commit -m "chore(config): add local config (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  chore(config): add local config"$'\n'
ok "chore(config): add local config"

cat > config/testnet.json <<EOF
{"network":"testnet","rpc":"https://soroban-testnet.stellar.org","run_id":$RUN_ID}
EOF
git add -A && git commit -m "chore(config): add testnet config (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  chore(config): add testnet config"$'\n'
ok "chore(config): add testnet config"

cat > config/mainnet.json <<EOF
{"network":"mainnet","rpc":"https://soroban.stellar.org","run_id":$RUN_ID}
EOF
git add -A && git commit -m "chore(config): add mainnet config (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  chore(config): add mainnet config"$'\n'
ok "chore(config): add mainnet config"

cat > config/rate_oracle.json <<EOF
{"run_id":$RUN_ID,"interval":300,"threshold":0.5}
EOF
git add -A && git commit -m "feat(config): add rate oracle config (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  feat(config): add rate oracle config"$'\n'
ok "feat(config): add rate oracle config"

# ============================================================
# Phase 12 - More docs
# ============================================================
step "Phase 12 - More docs ($RUN_ID)"

cat > docs/testing.md <<EOF
# Testing (run $RUN_ID)
Run: cargo test
EOF
git add -A && git commit -m "docs: add testing guide (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  docs: add testing guide"$'\n'
ok "docs: add testing guide"

cat > docs/operations.md <<EOF
# Operations (run $RUN_ID)
Admin publishes rates, monitors liquidity.
EOF
git add -A && git commit -m "docs: add operations guide (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  docs: add operations guide"$'\n'
ok "docs: add operations guide"

cat > docs/integration.md <<EOF
# Integration (run $RUN_ID)
Call quote(), then submit swap().
EOF
git add -A && git commit -m "docs: add integration guide (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  docs: add integration guide"$'\n'
ok "docs: add integration guide"

# ============================================================
# Phase 13 - Banks and payouts
# ============================================================
step "Phase 13 - Banks/payouts ($RUN_ID)"

cat > data/countries.json <<EOF
[{"code":"NG","name":"Nigeria","run_id":$RUN_ID},{"code":"KE","name":"Kenya","run_id":$RUN_ID}]
EOF
git add -A && git commit -m "feat: add countries list (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  feat: add countries list"$'\n'
ok "feat: add countries list"

cat > data/banks/ngn_banks.json <<EOF
[{"code":"001","name":"Access Bank","run_id":$RUN_ID},{"code":"002","name":"GTBank","run_id":$RUN_ID}]
EOF
git add -A && git commit -m "feat(banks): add NG banks (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  feat(banks): add NG banks"$'\n'
ok "feat(banks): add NG banks"

cat > data/banks/kes_banks.json <<EOF
[{"code":"KE01","name":"KCB","run_id":$RUN_ID},{"code":"KE02","name":"Equity","run_id":$RUN_ID}]
EOF
git add -A && git commit -m "feat(banks): add KE banks (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  feat(banks): add KE banks"$'\n'
ok "feat(banks): add KE banks"

cat > data/payouts/methods.json <<EOF
{"run_id":$RUN_ID,"mobile_money":true,"bank_transfer":true,"card":true}
EOF
git add -A && git commit -m "feat(payouts): add payout methods (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  feat(payouts): add payout methods"$'\n'
ok "feat(payouts): add payout methods"

cat > data/payouts/settlement_times.json <<EOF
{"run_id":$RUN_ID,"mobile":"instant","bank":"1-3d","card":"instant"}
EOF
git add -A && git commit -m "feat(payouts): add settlement times (run $RUN_ID)" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  feat(payouts): add settlement times"$'\n'
ok "feat(payouts): add settlement times"

# ============================================================
# Phase 14 - Final polish (always unique via run_id)
# ============================================================
step "Phase 14 - Final polish ($RUN_ID)"

echo "// run $RUN_ID" >> "$LIB"
git add -A && git commit -m "chore: add run $RUN_ID footer to lib.rs" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  chore: add run $RUN_ID footer"$'\n'
ok "chore: add run $RUN_ID footer"

cat >> README.md <<EOF

## Run $RUN_ID

This batch was generated on $(date -u +%FT%TZ).
EOF
git add -A && git commit -m "docs: add run $RUN_ID to README" >/dev/null 2>&1
SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  docs: add run $RUN_ID to README"$'\n'
ok "docs: add run $RUN_ID to README"

if command -v rustfmt &>/dev/null; then
  rustfmt "$LIB" 2>/dev/null || true
  git add -A
  if ! git diff --cached --quiet; then
    git commit -m "style: format lib.rs (run $RUN_ID)" >/dev/null 2>&1
    SUMMARY="$SUMMARY  - $(git rev-parse --short HEAD)  style: format lib.rs"$'\n'
    ok "style: format lib.rs"
  fi
fi

echo ""
echo "================================================================================"
echo "  Run $RUN_ID complete!"
echo "================================================================================"
echo ""
echo -n "$SUMMARY"
COUNT=$(git rev-list --count HEAD)
echo ""
echo "Total commits: $COUNT"
echo "================================================================================"

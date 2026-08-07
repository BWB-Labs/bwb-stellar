#!/bin/bash
# Deploy BWB Soroban contracts to Stellar Mainnet
# WARNING: This deploys to production. Requires audit completion.
# SC-X02: each contract is deployed+initialized atomically where the CLI
# supports it; the re-init guard prevents hijack during the deploy window.
set -euo pipefail

NETWORK="mainnet"

# SEC-A3: use a named keystore identity — never expose the seed as an argv token.
# Import once: printf '%s\n' "$SECRET_SEED" | soroban keys add deployer --secret-key
: "${DEPLOYER_IDENTITY:?DEPLOYER_IDENTITY must be set (name of the soroban keys entry, e.g. 'deployer')}"
: "${ADMIN_G:?ADMIN_G (admin cold-wallet Stellar G-address) must be set}"
: "${OPERATOR_G:?OPERATOR_G (operator hot-wallet Stellar G-address) must be set}"
: "${TOKEN_NAME:?TOKEN_NAME must be set}"
: "${TOKEN_SYMBOL:?TOKEN_SYMBOL must be set}"
: "${METADATA_JSON:?METADATA_JSON (XDR or JSON blob) must be set}"
: "${PAYOUT_ASSET_CONTRACT:?PAYOUT_ASSET_CONTRACT (payout asset contract address — USDC on Stellar) must be set}"

echo "=============================================="
echo "  BWB MAINNET DEPLOYMENT"
echo "  Network: Stellar Mainnet"
echo "  WARNING: This is a production deployment"
echo "=============================================="
echo ""

# Safety confirmation
read -r -p "Have you completed the security audit? (yes/no): " AUDIT_CONFIRMED
if [ "$AUDIT_CONFIRMED" != "yes" ]; then
  echo "Deployment cancelled. Complete security audit before mainnet deploy."
  exit 1
fi

read -r -p "Type 'DEPLOY MAINNET' to confirm: " CONFIRM
if [ "$CONFIRM" != "DEPLOY MAINNET" ]; then
  echo "Deployment cancelled."
  exit 1
fi

echo ""
echo "Deploying BWB contracts to Stellar Mainnet..."

# 1. KYC whitelist — atomic deploy+initialize (SC-X02)
echo "Deploying kyc-whitelist..."
KYC_ADDRESS=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/kyc_whitelist.wasm \
  --network "$NETWORK" \
  --source "$DEPLOYER_IDENTITY" \
  -- initialize \
  --admin "$ADMIN_G")
echo "kyc-whitelist: $KYC_ADDRESS"

# 2. Real-estate-token — atomic deploy+initialize (SEC-A1 / SC-X02)
echo "Deploying real-estate-token..."
RE_TOKEN_ADDRESS=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/real_estate_token.wasm \
  --network "$NETWORK" \
  --source "$DEPLOYER_IDENTITY" \
  -- initialize \
  --admin "$ADMIN_G" \
  --operator "$OPERATOR_G" \
  --kyc_contract "$KYC_ADDRESS" \
  --name "$TOKEN_NAME" \
  --symbol "$TOKEN_SYMBOL" \
  --metadata "$METADATA_JSON")
echo "real-estate-token: $RE_TOKEN_ADDRESS"

# 3. Distribution — atomic deploy+initialize (SC-X02)
echo "Deploying distribution..."
DIST_ADDRESS=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/distribution.wasm \
  --network "$NETWORK" \
  --source "$DEPLOYER_IDENTITY" \
  -- initialize \
  --admin "$ADMIN_G" \
  --token_contract "$RE_TOKEN_ADDRESS" \
  --kyc_contract "$KYC_ADDRESS" \
  --payout_asset "$PAYOUT_ASSET_CONTRACT")
echo "distribution: $DIST_ADDRESS"

# Save addresses
cat > deployed-addresses-mainnet.json <<EOF
{
  "network": "mainnet",
  "deployed_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "contracts": {
    "real_estate_token": "$RE_TOKEN_ADDRESS",
    "kyc_whitelist": "$KYC_ADDRESS",
    "distribution": "$DIST_ADDRESS"
  },
  "explorer": {
    "real_estate_token": "https://stellar.expert/explorer/public/contract/$RE_TOKEN_ADDRESS",
    "kyc_whitelist": "https://stellar.expert/explorer/public/contract/$KYC_ADDRESS",
    "distribution": "https://stellar.expert/explorer/public/contract/$DIST_ADDRESS"
  }
}
EOF

echo ""
echo "MAINNET DEPLOYMENT COMPLETE"
echo "Addresses saved to deployed-addresses-mainnet.json"

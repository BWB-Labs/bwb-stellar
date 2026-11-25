# Full Deployment Helper

Use `./shells/full-deploy.sh` for the end-to-end **testnet** flow (fund investor wallets, deploy the contracts against the BRLA token in `BRLA_ADDRESS`, configure allowlists). For a **mainnet** rollout that skips funding and allocation seeding, run `./shells/full-deploy-mainnet.sh`. Both scripts emit `configs/deployment-$DEPLOYMENT_ENVIRONMENT.json` so downstream tooling can resolve contract addresses — `full-deploy.sh` defaults `DEPLOYMENT_ENVIRONMENT` to `development`, `full-deploy-mainnet.sh` defaults it to `production`.

The Base Sepolia contract addresses live in `configs/deployment-base-sepolia.json`.

1. **Deploy the OfferTokenFactory (keystore account + explorer verification)**

   ```bash
   # assumes .env/.env.dev defines RPC_URL, CHAIN_ID, VERIFIER, VERIFIER_URL,
   # ETHERSCAN_API_KEY, and either MNEMONIC or PRIVATE_KEY

   forge script script/DeployOfferTokenFactory.s.sol:DeployOfferTokenFactory \
     --broadcast \
     --verify \
     --rpc-url "$RPC_URL" \
     --chain "$CHAIN_ID" \
     --verifier "$VERIFIER" \
     --verifier-url "$VERIFIER_URL" \
     --etherscan-api-key "$ETHERSCAN_API_KEY" \
     --account deployer
   ```

   > Forge will prompt for the keystore password if "deployer" is encrypted. To run via mnemonic/private key instead, replace the final flag with `--mnemonic "$MNEMONIC"` or `--private-key "$PRIVATE_KEY"`.

2. **Deploy an OfferERC20 via an existing OfferTokenFactory (keystore + always verify)**

   ```bash
   # point to the factory address you already deployed and load the keystore entry named "deployer"
   export FACTORY_ADDRESS=<OfferTokenFactory address>
   export TOKEN_NAME="Example Offer"
   export TOKEN_SYMBOL=OFFER
   # token owner can be specified, if not, the deployer is set as the owner
   #   export TOKEN_OWNER=0x2222222222222222222222222222222222222222
   export INITIAL_SUPPLY=1000000000000000000000000   # 1M tokens (18 decimals)
   export OFFER_URI="https://example.com/offers/example-token.json"

    forge script script/DeplorOfferERC20.s.sol:DeployOfferERC20 \
      --rpc-url "$RPC_URL" \
      --chain "$CHAIN_ID" \
      --broadcast \
      --verify \
      --verifier "$VERIFIER" \
      --verifier-url "$VERIFIER_URL" \
      --etherscan-api-key "$ETHERSCAN_API_KEY" \
      --account deployer
    # Forge prompts for the keystore password; keep the keystore unlocked to sign the
    # deployment and verification transactions without exposing the raw private key.
   ```

   > Tip: `./shells/create-offer-token.sh` reads `configs/deployment-$DEPLOYMENT_ENVIRONMENT.json`
   > (default `production`; or a path passed via `--config`) to populate the `OfferTokenFactory`
   > address before invoking the same Foundry script.

3. **Deploy the OfferSaleAllowlist (keystore + optional owner override)**

   ```bash
   # optionally force a specific allowlist owner; otherwise the deployer is used
   # export ALLOWLIST_OWNER=0x3333333333333333333333333333333333333333

   forge script script/DeployOfferSaleAllowlist.s.sol:DeployOfferSaleAllowlist \
     --rpc-url "$RPC_URL" \
     --chain "$CHAIN_ID" \
     --broadcast \
     --verify \
     --verifier "$VERIFIER" \
     --verifier-url "$VERIFIER_URL" \
     --etherscan-api-key "$ETHERSCAN_API_KEY" \
     --account deployer
   # The script falls back to the deployer as the owner when ALLOWLIST_OWNER is unset.
   ```

4. **Deploy the OfferTokenSaleFactory (keystore + explorer verification)**

   ```bash
   forge script script/DeployOfferTokenSaleFactory.s.sol:DeployOfferTokenSaleFactory \
     --rpc-url "$RPC_URL" \
     --chain "$CHAIN_ID" \
     --broadcast \
     --verify \
     --verifier "$VERIFIER" \
     --verifier-url "$VERIFIER_URL" \
     --etherscan-api-key "$ETHERSCAN_API_KEY" \
     --account deployer
   # Once deployed, interact with OfferTokenSaleFactory to spin up OfferTokenSale contracts that emit events for your subgraph.
   ```

5. **Deploy the TokenDistributorFactory (keystore + explorer verification)**

```bash
forge script script/DeployTokenDistributorFactory.s.sol:DeployTokenDistributorFactory \
  --rpc-url "$RPC_URL" \
  --chain "$CHAIN_ID" \
  --broadcast \
  --verify \
  --verifier "$VERIFIER" \
  --verifier-url "$VERIFIER_URL" \
  --etherscan-api-key "$ETHERSCAN_API_KEY" \
  --account deployer
# Constructor deploys a TokenDistributor implementation once; all future clones reuse it.
# Keep the keystore unlocked to sign both deployment and verification txs if using an encrypted account.
```

6. **Deploy an OfferTokenSale via an existing OfferTokenSaleFactory (env-driven params)**

    ```bash
    export TOKEN_SALE_FACTORY_ADDRESS=<OfferTokenSaleFactory address>
    export OFFER_TOKEN_ADDRESS=<OfferERC20 address>
    export BRLA_TOKEN_ADDRESS=<BRLA address>
    export BRLA_PER_TOKEN=1000000000000000000   # Price per offer token in BRLA wei
    export ALLOWLIST_ADDRESS=<OfferSaleAllowlist address>
    export SALE_HARD_CAP=100000000000000000000  # Maximum BRLA (wei) accepted
    export SALE_COOLING_OFF_PERIOD=432000       # Optional (seconds) – defaults to 5 days
    export SALE_MIN_SUCCESS_PERCENT=75          # Optional – defaults to 75%
    # optionally override the default owner (tx.origin)
    # export SALE_OWNER=0x8888888888888888888888888888888888888888

     forge script script/DeployOfferTokenSale.s.sol:DeployOfferTokenSale \
       --rpc-url "$RPC_URL" \
      --chain "$CHAIN_ID" \
      --broadcast \
      --verify \
      --verifier "$VERIFIER" \
      --verifier-url "$VERIFIER_URL" \
      --etherscan-api-key "$ETHERSCAN_API_KEY" \
      --account deployer
    # The script calls the factory with the provided params and logs the new sale address for indexing.
    # Ensure BRLA_PER_TOKEN evenly divides the BRLA amount buyers intend to spend.
   ```

   > Tip: `./shells/create-offer-token-sale.sh --offer-token <address> --price <wei>`
   > reads `configs/deployment-$DEPLOYMENT_ENVIRONMENT.json` (default `production`), deploys
   > the sale via the factory, and automatically approves + seeds the OfferTokenSale using
   > `script/SeedOfferTokenSale.s.sol`.

7. **Buy offer tokens from a deployed OfferTokenSale (keystore account signer)**

```bash
export OFFER_TOKEN_SALE_ADDRESS=<OfferTokenSale address>
export BRLA_AMOUNT=1000000000000000000   # 1 BRLA (18 decimals)

forge script script/BuyOfferTokens.s.sol:BuyOfferTokens \
  --rpc-url "$RPC_URL" \
  --chain "$CHAIN_ID" \
  --broadcast \
  --account buyer
# Configure the keystore entry named "buyer" (or adjust the --account flag) before running.
# The script previews the purchase, approves BRLA, and surfaces revert reasons for easier debugging.
# Ensure the buyer account holds enough BRLA and the sale contract has sufficient offer liquidity.
```

8. **Add buyers to an existing OfferSaleAllowlist**

```bash
# create a JSON payload describing buyers, allocations (18 decimals), and enabled flags
cat <<'EOF' > allowlist-payload.json
{
  "buyers": [
    "0x1111111111111111111111111111111111111111",
    "0x2222222222222222222222222222222222222222"
  ],
  "maxAllocations": [
    "1000000000000000000000",
    "500000000000000000000"
  ],
  "enabled": [
    true,
    true
  ]
}
EOF

chmod 666 allowlist-payload.json

export ALLOWLIST_ADDRESS=<OfferSaleAllowlist address>
export ALLOWLIST_PAYLOAD=allowlist-payload.json

forge script script/ConfigureOfferSaleAllowlist.s.sol:ConfigureOfferSaleAllowlist \
  --rpc-url "$RPC_URL" \
  --chain "$CHAIN_ID" \
  --broadcast \
  --account deployer
# The signer must be the OfferSaleAllowlist owner (or an account with ownership permissions). # maxAllocations values use 18 decimals (same as offer token). Enabled flags can selectively pause buyers without changing allocations.
```

## OfferTokenSaleFactory ownership transfer

### Required environment

Ensure your Forge account named `deployer` is unlocked and corresponds to the current factory owner.

### Dry run (simulation only)

Use a Base RPC endpoint to simulate the call without broadcasting. Provide the required environment variables inline with the command:

```
OFFER_TOKEN_SALE_FACTORY_ADDRESS=0x8308dD18bA9927567728E3d2Ce0955D6A3D68dd5 \
NEW_OFFER_TOKEN_SALE_FACTORY_OWNER=0x84EE2F3F5301074643E9020eCecC02d1389D3881 \
forge script script/calls/TransferOfferTokenSaleFactoryOwnership.s.sol \
  --rpc-url $RPC_URL \
  --account deployer
```

This executes the script against the provided RPC endpoint, logs the current owner, and simulates the ownership transfer without sending a transaction.

### Broadcast to Base mainnet

After reviewing the dry-run output, broadcast the transaction with the same inline arguments:

```
OFFER_TOKEN_SALE_FACTORY_ADDRESS=0x8308dD18bA9927567728E3d2Ce0955D6A3D68dd5 \
NEW_OFFER_TOKEN_SALE_FACTORY_OWNER=0x84EE2F3F5301074643E9020eCecC02d1389D3881 \
forge script script/calls/TransferOfferTokenSaleFactoryOwnership.s.sol \
  --rpc-url $RPC_URL \
  --broadcast \
  --account deployer
```

Foundry will prompt for the deployer keystore password (if any), submit the transaction, and display the resulting hash.

## OfferTokenFactory ownership transfer

### Required environment

Ensure your Forge account named `deployer` is unlocked and corresponds to the current factory owner.

### Dry run (simulation only)

```
OFFER_TOKEN_FACTORY_ADDRESS=0xBC2904b8C3b6cf6C882E68F34Fe0AD633B201009 \
NEW_OFFER_TOKEN_FACTORY_OWNER=0x84EE2F3F5301074643E9020eCecC02d1389D3881 \
forge script script/calls/TransferOfferTokenFactoryOwnership.s.sol \
  --rpc-url $RPC_URL \
  --account deployer
```

This simulates the ownership transfer without submitting a transaction.

### Broadcast to Base mainnet

```
OFFER_TOKEN_FACTORY_ADDRESS=0xBC2904b8C3b6cf6C882E68F34Fe0AD633B201009 \
NEW_OFFER_TOKEN_FACTORY_OWNER=0x84EE2F3F5301074643E9020eCecC02d1389D3881 \
forge script script/calls/TransferOfferTokenFactoryOwnership.s.sol \
  --rpc-url $RPC_URL \
  --broadcast \
  --account deployer
```

Foundry will log the existing owner, prompt for any password, and return the transaction hash once sent.

## TokenDistributorFactory ownership transfer

### Required environment

Ensure your Forge account named `deployer` is unlocked and corresponds to the current factory owner.

### Dry run (simulation only)

```
TOKEN_DISTRIBUTOR_FACTORY_ADDRESS=0xbaF31BAd298A93B2A62042fbFb03b50b95d2d9C0 \
NEW_TOKEN_DISTRIBUTOR_FACTORY_OWNER=0x84EE2F3F5301074643E9020eCecC02d1389D3881 \
forge script script/calls/TransferTokenDistributorFactoryOwnership.s.sol \
  --rpc-url $RPC_URL \
  --account deployer
```

This simulates the ownership transfer without sending a transaction.

### Broadcast to Base mainnet

```
TOKEN_DISTRIBUTOR_FACTORY_ADDRESS=0xbaF31BAd298A93B2A62042fbFb03b50b95d2d9C0 \
NEW_TOKEN_DISTRIBUTOR_FACTORY_OWNER=0x84EE2F3F5301074643E9020eCecC02d1389D3881 \
forge script script/calls/TransferTokenDistributorFactoryOwnership.s.sol \
  --rpc-url $RPC_URL \
  --broadcast \
  --account deployer
```

Foundry will prompt for any keystore credentials, submit the transaction, and print the resulting hash.

## OfferSaleAllowlist ownership transfer (proxy)

### Required environment

Ensure your Forge account named `deployer` is unlocked and corresponds to the current proxy owner. The script targets the proxy at `0x8d1Ea182C4003af8eF64d1DBb286903f5Cf2E859`, not the implementation.

### Dry run (simulation only)

```
OFFER_SALE_ALLOWLIST_PROXY_ADDRESS=0x8d1Ea182C4003af8eF64d1DBb286903f5Cf2E859 \
NEW_OFFER_SALE_ALLOWLIST_OWNER=0x84EE2F3F5301074643E9020eCecC02d1389D3881 \
forge script script/calls/TransferOfferSaleAllowlistOwnership.s.sol \
  --rpc-url $RPC_URL \
  --account deployer \
  -vvvv
```

This simulates the transfer, logs the current owner, and verifies the proxy has bytecode before proceeding.

### Broadcast to Base mainnet

```
OFFER_SALE_ALLOWLIST_PROXY_ADDRESS=0x8d1Ea182C4003af8eF64d1DBb286903f5Cf2E859 \
NEW_OFFER_SALE_ALLOWLIST_OWNER=0x84EE2F3F5301074643E9020eCecC02d1389D3881 \
forge script script/calls/TransferOfferSaleAllowlistOwnership.s.sol \
  --rpc-url $RPC_URL \
  --broadcast \
  --account deployer \
  -vvvv
```

The script re-reads `owner()` after the broadcast to ensure the Safe took control.

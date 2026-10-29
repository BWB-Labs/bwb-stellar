# @bwb/stellar-sdk

BWB's Stellar transaction layer. It provisions users' embedded wallets through Privy, opens their accounts and trustlines on reserves the platform sponsors, builds every envelope from a platform channel account, collects the signatures it needs, and submits it under a fee-bump the platform pays. On top of that it creates organizations' multisig treasuries, changes their signers, and deploys tokenized offerings.

Balance and payment reads are not part of this release.

## Modules

| Module | Exports | What it does |
|---|---|---|
| [`network`](src/network.ts) | `connect`, `NETWORK`, `passphraseOf`, `networkIdOf`, `usdcOf`, `accountKey` | An RPC connection that tries each endpoint in order and refuses one that serves another network. |
| [`signing`](src/signing.ts) | `HashSigner`, `privySigner`, `keypairSigner`, `isSignatureOf` | A signer is an address and a function that signs a hash. `privySigner` signs through Privy's raw signing, so no key ever leaves Privy. `isSignatureOf` checks a signature made in the browser before it is used. |
| [`wallet`](src/wallet.ts) | `ensureStellarWallet`, `walletOpeningOperations` | Reuses the user's Privy embedded Stellar wallet or pregenerates one, and builds the sponsored account creation and USDC trustline. |
| [`relay`](src/relay.ts) | `createRelay`, `RELAY_FAILURE` | `submitAsPlatform` for envelopes only platform keys sign; `prepareForSigners` and `submitSigned` for envelopes users or officers sign. Every envelope is sourced from a channel and fee-bumped by the sponsor. |
| [`treasury`](src/treasury.ts) | `treasuryCreationOperations`, `signerChangeOperations`, `readSignerSet` | An organization's treasury is a native multisig: its officers are its signers, at one threshold, with the master key disabled. |
| [`offering`](src/offering.ts) | `offeringTerms`, `deploymentDigest`, `deploymentOperation`, `isStepApplied`, `DEPLOYMENT_STEPS` | Derives an offering's contract addresses, the digest its officers sign, and the four deployment steps: token, sale, allowlist grant and activation. |
| [`soroban`](src/soroban.ts) | `createContractOperation`, `invokeContractOperation`, `prepareSorobanTransaction`, `simulateRead`, `contractExists`, `scv` | Contract calls: simulation, resource assembly and read-only views. |

Every function that can fail for an expected reason returns a `Result`: `{ success: true, data }` or `{ success: false, error: { code } }`. The relay's codes are:

| Code | Meaning |
|---|---|
| `preparationFailed` | The envelope could not be built |
| `simulationReverted` | A contract call failed its simulation |
| `signingFailed` | A signer did not sign, or a signature is not of the envelope; nothing was sent |
| `relayAccountMissing` | The channel or source account does not exist on chain |
| `submissionFailed` | The envelope was sent and may still land; `transactionHash` names it |
| `timeBoundPassed` | The envelope can no longer land |
| `transactionFailed` | The network applied and refused it; `providerMessage` holds the result code |

## Usage

```ts
import { connect, createRelay, NETWORK, privySigner, usdcOf, walletOpeningOperations } from '@bwb/stellar-sdk'

const rpc = connect({ network: NETWORK.TESTNET, rpcUrls: [alchemyUrl, 'https://soroban-testnet.stellar.org'] })
const sponsor = privySigner(privy.wallets(), { walletId: sponsorWalletId, address: sponsorAddress })
const relay = createRelay({ rpc, sponsor })

const prepared = await relay.prepareForSigners({
  source: channelAddress,
  operations: walletOpeningOperations({
    address: investorAddress,
    sponsor: sponsor.address,
    asset: usdcOf(NETWORK.TESTNET).asset,
    opensAccount: true,
  }),
  timeBound: Math.floor(Date.now() / 1000) + 180,
})
// The investor signs prepared.data.hash in the browser with their embedded wallet.
const submitted = await relay.submitSigned({
  envelope: prepared.data.envelope,
  signatures: [{ address: investorAddress, signature }],
  platformSigners: [channel, sponsor],
})
```

A channel is leased to one envelope at a time, so concurrent envelopes never share a sequence number. The caller keeps a channel leased until its envelope's time bound when the relay answers `submissionFailed`.

## Testnet configuration

| Setting | Value |
|---|---|
| Network | Stellar testnet, `Test SDF Network ; September 2015` |
| RPC endpoints, in order | Alchemy's Stellar testnet endpoint, then `https://soroban-testnet.stellar.org` |
| Sponsor | [`GBWAEFXNZ45DTLZNDTI4UY65OX2BPWYQ6MIDTH5IS4XTQ3RMWVHQ7NXZ`](https://stellar.expert/explorer/testnet/account/GBWAEFXNZ45DTLZNDTI4UY65OX2BPWYQ6MIDTH5IS4XTQ3RMWVHQ7NXZ) |
| Channels | [`GCVR7GJG…`](https://stellar.expert/explorer/testnet/account/GCVR7GJGE3UQJJYJO5CTWXLSL2RTH6BEWNZYOXV743H7KUW4KDHYECNT), [`GAJDCR3I…`](https://stellar.expert/explorer/testnet/account/GAJDCR3IN4DWCSUBUZNGWSRHIQJVWYEIESRVAXGLLIPE7ZLP6IR2U776), [`GDU3CAV3…`](https://stellar.expert/explorer/testnet/account/GDU3CAV35P7ECHL3LH7MGK4CANH5YFHRYKMBWRI3FUBUGNBVTNFBYDOY), [`GAK25MX7…`](https://stellar.expert/explorer/testnet/account/GAK25MX77HDIC7QREV67IQ6RVJFPLMCQHNGQ2LCPVEDVJQQWAPM5RAKW), [`GBTOAPT5…`](https://stellar.expert/explorer/testnet/account/GBTOAPT5J7I2VJGQE3DU75TMGREVHEHDJ7GKDTJ2XSX33IBTOAEHOQ4T) |
| Platform deployer | [`GCSWSNEMQ7GBTBADWHO3KRWNAHXKBNF7JWSODOVRPZWJOFQMNPVNI73Z`](https://stellar.expert/explorer/testnet/account/GCSWSNEMQ7GBTBADWHO3KRWNAHXKBNF7JWSODOVRPZWJOFQMNPVNI73Z) |
| Platform admin (pauser, upgrader) | [`GC7CI4BAER3PTFCAG6WRJ7WZI4DOWBN27RZQXDBFHBRRCCXQBSY4ZUHZ`](https://stellar.expert/explorer/testnet/account/GC7CI4BAER3PTFCAG6WRJ7WZI4DOWBN27RZQXDBFHBRRCCXQBSY4ZUHZ) |
| USDC | Circle's testnet USDC, issuer `GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5`, contract `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA` |
| Offer allowlist | [`CDT4GEAJ5XEZUG2O5LZH6G6ZVJMDXC7K3WITDZJQ4FUIR2MZNKEHO6BE`](https://stellar.expert/explorer/testnet/contract/CDT4GEAJ5XEZUG2O5LZH6G6ZVJMDXC7K3WITDZJQ4FUIR2MZNKEHO6BE) |
| `offer-token` WASM hash | `5e4b5c74143e5434597efcfb6f2d3c5b7ab45a5ad6bef9405e8e710e5c3caf25` |
| `offer-sale` WASM hash | `3abb426caf7d60217c29f96bd2dd73acf60fd636f88c67f686967613acfa34d9` |
| Signing window | 180 seconds for a user's signature |

The sponsor, the channels and the deployer are keys Privy holds. BWB stores none of them.

## Reproduce

Node 22:

```bash
npm ci --prefix sdk
npm run typecheck --prefix sdk
npm test --prefix sdk
```

The suites run against a stubbed Stellar RPC, so they need no network and no keys. CI runs the same commands on every pull request that touches `sdk/` ([workflow](../.github/workflows/sdk.yml)).

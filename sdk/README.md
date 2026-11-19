# @bwb-labs/sdk

BWB's Stellar transaction layer. It provisions users' embedded wallets through Privy, opens their accounts and trustlines on reserves the platform sponsors, builds every envelope from a platform channel account, collects the signatures it needs, and submits it under a fee-bump the platform pays. On top of that it creates organizations' multisig treasuries, changes their signers, and deploys tokenized offerings.

Balance and payment reads are not part of this release.

## Modules

| Module | Exports | What it does |
|---|---|---|
| [`network`](src/stellar/network.ts) | `connect`, `NETWORK`, `passphraseOf`, `networkIdOf`, `usdcOf`, `accountKey` | An RPC connection that tries each endpoint in order and refuses one that serves another network. |
| [`signing`](src/stellar/signing.ts) | `HashSigner`, `privySigner`, `keypairSigner`, `isSignatureOf` | A signer is an address and a function that signs a hash. `privySigner` signs through Privy's raw signing, so no key ever leaves Privy. `isSignatureOf` checks a signature made in the browser before it is used. |
| [`wallet`](src/stellar/wallet.ts) | `ensureStellarWallet`, `walletOpeningOperations` | Reuses the user's Privy embedded Stellar wallet or pregenerates one, and builds the sponsored account creation and USDC trustline. |
| [`relay`](src/stellar/relay.ts) | `createRelay`, `RELAY_FAILURE` | `submitAsPlatform` for envelopes only platform keys sign; `prepareForSigners` and `submitSigned` for envelopes users or officers sign. Every envelope is sourced from a channel and fee-bumped by the sponsor. |
| [`treasury`](src/stellar/treasury.ts) | `treasuryCreationOperations`, `signerChangeOperations`, `readSignerSet` | An organization's treasury is a native multisig: its officers are its signers, at one threshold, with the master key disabled. |
| [`offering`](src/stellar/offering.ts) | `offeringTerms`, `deploymentDigest`, `deploymentOperation`, `isStepApplied`, `DEPLOYMENT_STEPS` | Derives an offering's contract addresses, the digest its officers sign, and the four deployment steps: token, sale, allowlist grant and activation. |
| [`soroban`](src/stellar/soroban.ts) | `createContractOperation`, `invokeContractOperation`, `prepareSorobanTransaction`, `simulateRead`, `contractExists`, `scv` | Contract calls: simulation, resource assembly and read-only views. |

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

## Buying quotas

One sale interface serves every chain. The configuration holds a client for each chain id a deployment serves, and the SDK returns the strategy for the offering's chain id, bound to that offering. Each chain is named by its own identifier: a Stellar chain by its network id, the SHA-256 of its passphrase, and a Base chain by its EVM chain id. The client must serve that chain, or every call returns `chainMismatch`. The caller builds the client and owns any fallback across endpoints; `connect` is one way to build it.

| Constant | Chain id | Strategy |
|---|---|---|
| `CHAIN_ID.STELLAR_TESTNET` | `cee0302d…ecd472`, the testnet network id | Stellar |
| `CHAIN_ID.STELLAR_MAINNET` | `7ac33997…45a979`, the mainnet network id | Stellar |
| `CHAIN_ID.BASE_TESTNET` | `84532` | None yet, refused with `chainNotSupported` |
| `CHAIN_ID.BASE_MAINNET` | `8453` | None yet, refused with `chainNotSupported` |

```ts
import { CHAIN_ID, connect, createBwbSdk, NETWORK } from '@bwb-labs/sdk'

const rpc = connect({ network: NETWORK.TESTNET, rpcUrls: ['https://soroban-testnet.stellar.org'] })
const bwb = createBwbSdk({ chains: { [CHAIN_ID.STELLAR_TESTNET]: { rpc } } })

const client = bwb.select(CHAIN_ID.STELLAR_TESTNET)
if (!client.success) return client
const sale = client.data.sale({ address: saleAddress })

const prepared = await sale.buy({ buyer, amount })
if (!prepared.success) return prepared
// The buyer signs prepared.data.entryHash with their wallet.
const authorized = await sale.authorize({ prepared: prepared.data, signature })
if (!authorized.success) return authorized
// Relay authorized.data.invocation, then read the purchase from the settled transaction.
const receipt = sale.purchaseReceipt({ buyer, transaction: { chainId: CHAIN_ID.STELLAR_TESTNET, resultMeta } })
```

`select` returns a client bound to one chain, so its `sale` takes only the offering's address. `bwb.sale({ chainId, address })` does both steps at once and returns the same refusals as `select`.

`buy` returns one authorization entry covering the purchase and its USDC transfer. `authorize` checks the signature against that entry and simulates the signed call again, so the relay assembles resources that cover signature verification. The source of the relayed transaction must not be the buyer.

| Refusal | Meaning |
|---|---|
| `chainNotConfigured` | The configuration holds no client for the offering's chain id |
| `chainNotSupported` | The SDK has no strategy for that chain id yet |
| `chainMismatch` | The configured client reports another network id than its chain id |
| `chainUnavailable` | No endpoint answered |
| `invalidBuyer` | The buyer is not an account address |
| `invalidTreasury` | The treasury is not an account address, or holds no account on chain |
| `unexpectedResponse` | The address did not answer like a sale |
| `unreadableTransaction` | The settled transaction is malformed or in a format this version cannot read |
| `investorNotEnabled` | The investor is not on the allowlist |
| `allocationExceeded` | The purchase exceeds the investor's allocation this period |
| `tooManyReceipts` | The investor holds the most live receipts this sale allows |
| `saleNotActive` | The sale is not open for buying, so it cannot be closed either |
| `belowMinimum` | The sale has not sold its minimum, so it cannot close; cancel it instead |
| `notWholeQuotas` | The amount is not an exact multiple of the price |
| `exceedsSupply` | Fewer quotas are left than the amount buys |
| `tooManyPurchases` | The investor holds the most live purchases this offering allows |
| `insufficientBalance` | The buyer holds too little of the payment asset |
| `balanceFrozen` | The asset's issuer froze the balance |
| `trustlineMissing` | The buyer has no trustline for the payment asset |
| `entryExpired` | The signed entry reached its expiration ledger; prepare the purchase again |
| `signatureMismatch` | A signature is not over this entry as prepared, or not by the buyer or a signer of the treasury |
| `belowThreshold` | Fewer officers signed the close than the treasury's threshold |
| `unexpectedAuthorization` | The simulation asked for something other than one entry covering this purchase or close, or a prepared close no longer holds it |
| `purchaseNotFound` | The settled transaction holds no purchase by this buyer in this sale |
| `contractRefused` | The contract refused for another reason; the message carries its error |

## Closing a sale

The treasury that owns a sale closes it, and its officers co-sign one authorization entry. A sale that sells out closes during its last purchase, with no entry to sign.

```ts
const closing = await sale.close({ treasury })
if (!closing.success) return closing
// Each officer signs closing.data.entryHash, within closing.data.expirationLedger.
const authorized = await sale.authorizeClose({ prepared: closing.data, signatures })
if (!authorized.success) return authorized
// Relay authorized.data.invocation.
```

`close` simulates the close and returns the one entry the treasury signs. `authorizeClose` takes one `{ address, signature }` per officer, each a `0x`-prefixed hex ed25519 signature over the entry hash. Without asking the chain, it refuses an entry that is not this treasury's close of this sale, and a signature that is not over the entry as prepared, expiration ledger included. It then reads the treasury's signers and refuses a signature by anyone else, or fewer distinct officers than the treasury's threshold, before anything is simulated. Last, it puts every signature on the entry in raw public key order and simulates the signed close again. An entry lives 360 ledgers, about thirty minutes.

Once the sale is Closing, `read()` reports `finalizableAt`: the time, in Unix milliseconds, from which the sale can be finalized and its distribution begin. The other states carry no such field.

## Usage

```ts
import { connect, createRelay, NETWORK, privySigner, usdcOf, walletOpeningOperations } from '@bwb-labs/sdk'

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
| `offer-token` WASM hash | `a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0` |
| `offer-sale` WASM hash | `49edfc275630882ce01e9a190a84f0f833ad3e48183ddd17ea2487463b0af6bd` |
| Signing window | 180 seconds for a user's signature |

The sponsor, the channels and the deployer are keys Privy holds. BWB stores none of them.

## Reproduce

Bun 1.4.4, from `sdk/`:

```bash
bun install --frozen-lockfile
bun run typecheck
bun run test
```

`bun run test` runs the vitest suite; Bun's own `bun test` runner is not used.

The suites run against a stubbed Stellar RPC, so they need no network and no keys. CI runs the same commands on every pull request that touches `sdk/` ([workflow](../.github/workflows/sdk.yml)).

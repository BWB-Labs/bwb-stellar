# SCF #45 · Tranche 1 · Deliverable 1 evidence

**Deliverable 1 — Embedded Stellar Wallet and Transaction Foundation.** This document takes each of D1's four completion criteria, as written in the [awarded submission](https://communityfund.stellar.org/submissions/rec8bhUAafIcSJUzA), and places next to it the code, tests, accounts and transactions that meet it. Each piece of evidence is linked, and each criterion ends with the way to reproduce it.

Everything runs on Stellar testnet. The source release is tag `t1` on `main`, commit `TODO(commit)`. The application itself, its Convex backend and its web client live in BWB's private repository; the Stellar integration code that D1 delivers is mirrored here, unchanged, under [`integration/`](../../integration), from private commit `TODO(private commit)`.

## The integration

D1 is the wallet and transaction layer every Stellar flow on the platform stands on. It is built as a set of capabilities, each an interface the backend calls with a Stellar adapter behind it, so business code never builds a transaction itself.

| Scope part | Capability | What it does |
|---|---|---|
| Embedded wallets and key management | Provisioning | Gives each user a Stellar key held by Privy, created from the server or reused when the user already has one. BWB receives the address, never the key. |
| Account creation and trustlines | Wallet provisioning, treasury provisioning | Opens an investor's account, or an organization's multisig treasury, in one transaction that also sets up the USDC trustline, with every reserve sponsored by the platform. |
| Unsigned XDR for user authorization | Relay | Builds every envelope from a platform channel account, hands its hash to the people who must sign it, checks each signature against the envelope, then submits it under a fee-bump. |
| Treasury signers | Treasury | Adds and removes a treasury's officers and moves its thresholds, through envelopes the current officers sign. |
| Offering deployment | Offering deployment | Deploys an offering's token and sale contracts under the terms the treasury's officers signed (the contracts are Deliverable 2). |
| Payment notifications | Movement notifications | Receives Alchemy's signed notifications of payments into the platform's Stellar accounts. |

Three kinds of key take part. A user's key signs in the browser through Privy's embedded wallet, behind the multi-factor authentication Privy enforces on every use of that key. The platform's keys, also held by Privy, sign as the sponsor that pays every fee and reserve and as the channel accounts that supply sequence numbers. An organization's treasury is a native Stellar multisig whose signers are its officers' own user keys, with the master key disabled.

The platform sources every Stellar transaction, so no user ever needs to hold XLM.

## Criterion 1 — met ✅

> A tagged public repository release contains the wallet integration, XDR builder interfaces, deployment/configuration manifest, reproduction instructions, and public CI results.

### Evidence

| Part | Where |
|---|---|
| Wallet integration | [`integration/providers/provisioning/`](../../integration/providers/provisioning) (Privy key pregeneration), [`integration/providers/walletProvisioning/`](../../integration/providers/walletProvisioning) (the investor's account) and [`integration/providers/treasuryProvisioning/`](../../integration/providers/treasuryProvisioning) (the organization's multisig) |
| XDR builder interfaces | [`Relay.ts`](../../integration/providers/relay/Relay.ts) and its Stellar adapter [`StellarRelay.ts`](../../integration/providers/relay/StellarRelay.ts): the channel-sourced envelope, the signature check and the fee-bump. [`Treasury.ts`](../../integration/providers/treasury/Treasury.ts) and [`StellarTreasury.ts`](../../integration/providers/treasury/StellarTreasury.ts): signer changes. [`soroban.ts`](../../integration/shared/utils/soroban.ts): contract creation and invocation, simulation and assembly |
| Deployment and configuration manifest | [`integration/configuration.md`](../../integration/configuration.md): every setting the backend reads on Stellar testnet, with the platform accounts below. Contracts: [`deployments/testnet.json`](../../deployments/testnet.json) |
| Reproduction instructions | [`integration/README.md`](../../integration/README.md) |
| Public CI | the [integration workflow](../../.github/workflows/integration.yml), [run on the release](TODO(run link)) |

The platform's accounts on testnet:

| Role | Account |
|---|---|
| Sponsor: pays every fee-bump and every sponsored reserve | [`GBWAEFXN…Q7NXZ`](https://stellar.expert/explorer/testnet/account/GBWAEFXNZ45DTLZNDTI4UY65OX2BPWYQ6MIDTH5IS4XTQ3RMWVHQ7NXZ) |
| Channel 1 | [`GCVR7GJG…YECNT`](https://stellar.expert/explorer/testnet/account/GCVR7GJGE3UQJJYJO5CTWXLSL2RTH6BEWNZYOXV743H7KUW4KDHYECNT) |
| Channel 2 | [`GAJDCR3I…2U776`](https://stellar.expert/explorer/testnet/account/GAJDCR3IN4DWCSUBUZNGWSRHIQJVWYEIESRVAXGLLIPE7ZLP6IR2U776) |
| Channel 3 | [`GDU3CAV3…BYDOY`](https://stellar.expert/explorer/testnet/account/GDU3CAV35P7ECHL3LH7MGK4CANH5YFHRYKMBWRI3FUBUGNBVTNFBYDOY) |
| Channel 4 | [`GAK25MX7…PM5RAKW`](https://stellar.expert/explorer/testnet/account/GAK25MX77HDIC7QREV67IQ6RVJFPLMCQHNGQ2LCPVEDVJQQWAPM5RAKW) |
| Channel 5 | [`GBTOAPT5…AEHOQ4T`](https://stellar.expert/explorer/testnet/account/GBTOAPT5J7I2VJGQE3DU75TMGREVHEHDJ7GKDTJ2XSX33IBTOAEHOQ4T) |

### How to verify

`npm ci --prefix integration && npm test --prefix integration` runs the mirrored suites with Node 22. The CI run linked above shows the same commands passing on GitHub's runners.

## Criterion 2 — TODO(status)

> Published Stellar testnet accounts and explorer links demonstrate investor and developer wallet creation/connection, balances, and transaction history without BWB storing private keys.

### Evidence

**Investor wallet.** An investor sets up their Stellar wallet from the portfolio page. The backend prepares the account's creation, the investor signs its hash in the browser with their embedded wallet, and the backend checks that signature against the address before submitting.

| | |
|---|---|
| Video | `TODO(video: investor wallet setup)` |
| Account | [`GAD265JN…SQK4G3ZW`](https://stellar.expert/explorer/testnet/account/GAD265JNWRYS72GGKZ37LAFBEJ6ZQNJYRCDU36OEVWQ3QZ5ISQK4G3ZW), whose only signer is its own key, held by Privy |
| Creation | [`05165b8c…`](https://stellar.expert/explorer/testnet/tx/05165b8cb7eb7088ddd7688feaa195b7c1ac9a74944f78bfba7348f6f695aaa3) |

**Developer wallet.** A developer is an organization that tokenizes real estate, and its wallet is the organization's treasury: a multisig whose signers are its officers.

| | |
|---|---|
| Video | `TODO(video 1: treasury provisioning)` |
| Account | [`GD53ISTA…EOLCPJ`](https://stellar.expert/explorer/testnet/account/GD53ISTANEJSWPW5TMVB3T4MXR6KHXDUALQCJUT47SKER546CQEOLCPJ): two officers at weight 1, every threshold 2, master key weight 0 |
| Creation | [`2fcd1aca…`](https://stellar.expert/explorer/testnet/tx/2fcd1aca55995950135f2500ab38b941eddc9bb694b6bdbe20b42d953c392ac1) |

**Treasury signers.** An officer proposes a signer change, the current officers approve it by signing its envelope, and it executes at the threshold.

| | |
|---|---|
| Video | `TODO(video 2: managing signers)` |
| Signer added, thresholds raised to 2 | [`6e734f6e…`](https://stellar.expert/explorer/testnet/tx/6e734f6ecab8cd85610b0c0063a76618bb799224c659ef63b2409f28a8a6663f) |
| Signer removed, thresholds lowered to 1 | [`3190667e…`](https://stellar.expert/explorer/testnet/tx/3190667ed8ef835d64ad8f07c46356f325e1edc2e9efdfe0d2be15afd019626a) |

**Offering deployment.** The treasury's officers sign the terms of an offering, and the platform deploys its token and sale contracts under them.

| | |
|---|---|
| Video | `TODO(video 3: deploying a tokenized offering)` |
| Deployments | `TODO(deploy hashes from video 3)` |

Each account's page on the explorer shows its balances, the sponsor of each reserve, its signers and its full transaction history.

**No private keys at BWB.** For a user, the backend stores the Stellar address and nothing else. For the platform's own accounts, it stores the address and the identifier of the key Privy holds. Users sign in the browser through Privy, and the platform signs by asking Privy to sign a hash. The tests that pin this down are in [`PrivyStellarProvisioning.test.ts`](../../integration/providers/provisioning/__tests__/PrivyStellarProvisioning.test.ts): *creates a Stellar wallet owned by the user, with no other signer, when Privy holds none*; *returns the Stellar wallet Privy already holds for the user and creates none*; *does not take a Stellar wallet the user connected from outside Privy*.

### How to verify

Open any account above on the explorer. The Signers panel lists the keys that control it, and none of them on a user's account is a BWB key. The History tab lists every transaction.

## Criterion 3 — met ✅

> Published transaction hashes demonstrate reserve sponsorship, trustline creation, and fee-bump execution on testnet.

### Evidence

One transaction shows all three. The investor wallet's creation, [`05165b8c…`](https://stellar.expert/explorer/testnet/tx/05165b8cb7eb7088ddd7688feaa195b7c1ac9a74944f78bfba7348f6f695aaa3), is a fee-bump paid by the sponsor around an inner transaction, [`92b46f30…`](https://stellar.expert/explorer/testnet/tx/92b46f308cd0e026b7df90be15ed444ebcaab2ec774bccb1323a3f5f0781e03e), sourced from channel 1:

| Operation | Source | What it shows |
|---|---|---|
| `begin_sponsoring_future_reserves` | sponsor | Reserve sponsorship opens for the new account |
| `create_account`, starting balance 0 | sponsor | The account exists with no XLM of its own |
| `change_trust` USDC | the investor | Trustline creation, signed by the investor's key |
| `end_sponsoring_future_reserves` | the investor | The investor accepts the sponsorship |

A second investor wallet was created the same way in [`638c6874…`](https://stellar.expert/explorer/testnet/tx/638c687434e10b0d92324b6c55b1b2d795991feed3245b059b241fe4556a4e9a). The treasury's creation sponsors its signers' reserves as well, and every later envelope is fee-bumped too: the signer removal [`3190667e…`](https://stellar.expert/explorer/testnet/tx/3190667ed8ef835d64ad8f07c46356f325e1edc2e9efdfe0d2be15afd019626a) and the contract deployments of criterion 2.

RPC keeps transactions for about seven days and testnet resets from time to time, so the decoded record of each transaction above is committed under [`evidence/d1/`](evidence/d1/).

### How to verify

Open the transaction on the explorer. The header names the fee-bump and its fee source, and the operations list shows the sponsorship, the account and the trustline. The account's page shows the sponsor next to its base reserve and its USDC trustline.

## Criterion 4 — TODO(status)

> Public CI evidence covers duplicate submission, timeout, retry, and rejected-signature scenarios, with every criterion mapped in the public evidence index.

### Evidence

The [integration workflow](../../.github/workflows/integration.yml) runs the mirrored suites on every pull request ([run on the release](TODO(run link))). The relay, which every Stellar envelope passes through, covers each scenario, and the flow that uses it covers it again:

| Scenario | Relay ([`StellarRelay.test.ts`](../../integration/providers/relay/__tests__/StellarRelay.test.ts)) | At the flow |
|---|---|---|
| Duplicate submission | *sends nothing and reports the outcome when the envelope already landed* | wallet setup: *returns the provisioning already under way instead of opening a second one*; treasury: *creates nothing again once the treasury records its Stellar address* |
| Timeout | *refuses with time bound passed once the chain has closed past it without the transaction*; *says the envelope may still land when it has not landed within a minute* | signer change ([`StellarTreasury.test.ts`](../../integration/providers/treasury/__tests__/StellarTreasury.test.ts)): *fails the change as past its deadline when the network never saw it by then*; wallet setup: *refuses a signature once the signing window has closed* |
| Retry | *reads the sequence again and resubmits when the chain refuses a stale one* | treasury: *retries a creation that fails on a transient error until it completes*; RPC: *falls back to the public endpoint when the managed one does not answer* |
| Rejected signature | *refuses with signing failed, sending nothing, when an officer’s signature is not of the envelope* | wallet setup: *refuses a signature made by another key than the wallet’s Stellar address* |

The flow-level tests run in BWB's private CI against the full backend. They are listed by title so each scenario can be traced; the relay and adapter tests are the ones that run here.

This document is the evidence index: each of the four criteria above, with its code, tests, accounts and transactions.

### How to verify

`npm test --prefix integration` runs the relay suite with the rest. The CI run linked above shows it passing.

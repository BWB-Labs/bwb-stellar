# SCF #45 · Tranche 1 · Deliverable 1 evidence

**Deliverable 1 — Embedded Stellar Wallet and Transaction Foundation.** This document takes each of D1's four completion criteria, as written in the [awarded submission](https://communityfund.stellar.org/submissions/rec8bhUAafIcSJUzA), and places next to it the code, tests, accounts and transactions that meet it. Each piece of evidence is linked, and each criterion ends with the command or the page that reproduces it.

Everything runs on Stellar testnet. The source release is tag [`t1`](https://github.com/BWB-Labs/bwb-stellar/tree/t1).

## The SDK

D1 is delivered as [`@bwb/stellar-sdk`](../../sdk), the Stellar transaction layer of BWB's platform. Four flows go through it: setting up an investor's wallet, creating an organization's treasury, changing its signers, and deploying an offering.

| Scope part | Module | What it does |
|---|---|---|
| Embedded wallets and key management | [`wallet`](../../sdk/src/wallet.ts), [`signing`](../../sdk/src/signing.ts) | Reuses the user's Privy embedded Stellar wallet or pregenerates one. Signs through Privy's raw signing, so keys never leave Privy, and checks every signature made in the browser against its address. |
| Account creation and trustlines | [`wallet`](../../sdk/src/wallet.ts), [`treasury`](../../sdk/src/treasury.ts) | Opens an investor's account, or an organization's multisig treasury, in one transaction that also adds the USDC trustline, on reserves the platform sponsors. |
| Unsigned XDR for user authorization | [`relay`](../../sdk/src/relay.ts) | Builds every envelope from a platform channel account and hands its hash to whoever must sign it. Once signed, it adds the platform's signatures, wraps the envelope in a fee-bump and submits it. |
| Fee sponsorship | [`relay`](../../sdk/src/relay.ts) | The sponsor pays the fee of every envelope through the fee-bump. Channel accounts supply sequence numbers only, so users never hold XLM. |
| Treasury signers | [`treasury`](../../sdk/src/treasury.ts) | Adds and removes a treasury's officers and moves its thresholds, through envelopes the current officers sign. |
| Offering deployment | [`offering`](../../sdk/src/offering.ts), [`soroban`](../../sdk/src/soroban.ts) | Derives an offering's contract addresses and the digest its officers sign, then deploys the token and sale contracts of Deliverable 2, grants the sale its allowlist role and activates it. |

Three parties sign. An investor or an officer signs in the browser with their Privy embedded wallet. Once a user has enrolled in multi-factor authentication, Privy enforces it on every use of that key, and the platform requires enrollment before any treasury action. An organization's treasury is a native Stellar multisig whose signers are its officers' own keys, with the master key disabled. The platform signs through keys Privy also holds: the sponsor, five channel accounts and the deployer. The SDK's [README](../../sdk/README.md) documents each module, the failure codes and the testnet configuration.

## Criterion 1 — met ✅

> A tagged public repository release contains the wallet integration, XDR builder interfaces, deployment/configuration manifest, reproduction instructions, and public CI results.

### Evidence

| Part | Where |
|---|---|
| Wallet integration | [`sdk/src/wallet.ts`](../../sdk/src/wallet.ts): `ensureStellarWallet` provisions the user's embedded Stellar wallet through Privy. [`sdk/src/signing.ts`](../../sdk/src/signing.ts): `privySigner` signs through Privy, `isSignatureOf` checks a browser signature. |
| XDR builder interfaces | [`sdk/src/relay.ts`](../../sdk/src/relay.ts): the `Relay` interface, `submitAsPlatform`, `prepareForSigners` and `submitSigned`. The operation builders `walletOpeningOperations`, `treasuryCreationOperations`, `signerChangeOperations` and `deploymentOperation`, in [`wallet`](../../sdk/src/wallet.ts), [`treasury`](../../sdk/src/treasury.ts) and [`offering`](../../sdk/src/offering.ts). |
| Deployment and configuration manifest | [Testnet configuration](../../sdk/README.md#testnet-configuration): network, RPC endpoints in their fallback order, sponsor, channels, deployer, admin, USDC and offering contracts. Contract deployments: [`deployments/testnet.json`](../../deployments/testnet.json). |
| Reproduction instructions | [SDK README, Reproduce](../../sdk/README.md#reproduce) |
| Public CI | the [SDK workflow](../../.github/workflows/sdk.yml): typecheck, build and the test suite, [run on this branch](https://github.com/BWB-Labs/bwb-stellar/actions/runs/40923689913) |

### How to verify

Check out the release commit and run, with Bun 1.4.4:

```bash
cd sdk && bun install --frozen-lockfile && bun run typecheck && bun run test
```

The suites run against a stubbed Stellar RPC, so they need neither network access nor keys.

## Criterion 2 — met ✅

> Published Stellar testnet accounts and explorer links demonstrate investor and developer wallet creation/connection, balances, and transaction history without BWB storing private keys.

### Evidence

**Investor wallet.** An investor sets up their Stellar wallet from the portfolio page. The platform prepares the account's creation with `prepareForSigners`, the investor signs its hash with their embedded wallet, and `submitSigned` checks the signature, adds the channel's and the sponsor's, and submits.

| | |
|---|---|
| Account | [`GAD265JN…SQK4G3ZW`](https://stellar.expert/explorer/testnet/account/GAD265JNWRYS72GGKZ37LAFBEJ6ZQNJYRCDU36OEVWQ3QZ5ISQK4G3ZW). Its only signer is its own key, held by Privy |
| Creation | [`05165b8c…`](https://stellar.expert/explorer/testnet/tx/05165b8cb7eb7088ddd7688feaa195b7c1ac9a74944f78bfba7348f6f695aaa3) |

**Developer wallet and its signers.** A developer is an organization that tokenizes real estate, and its wallet is the organization's treasury: a multisig whose signers are its officers. In the video, KMGN Holding's officer adds a second officer as a signer, which raises every threshold to 2, and the two officers then remove that signer together, each signing from a separate session.

| | |
|---|---|
| Video | [▶ Treasury and signers on Stellar](https://youtu.be/vgZ2SPrYcqI) (2:54) |
| Treasury | [`GCDF7SCT…NQXDSGHZ`](https://stellar.expert/explorer/testnet/account/GCDF7SCTOZFESDHDNKXUTSOVV6JPYHQPQD7YHFFFHBW4JGBBNQXDSGHZ): officers at weight 1, every threshold at the treasury's, master key weight 0 |
| Creation | [`cd1e1ae7…`](https://stellar.expert/explorer/testnet/tx/cd1e1ae73474660d02c1fbf5d933139909f5fbb0ca48ee0a2b9aa35d94cce22b): sponsored account, USDC trustline, officers as signers, master key disabled. It predates the fee-bump, so a channel account paid its fee |
| Signer added, thresholds raised to 2 | [`1aff29a2…`](https://stellar.expert/explorer/testnet/tx/1aff29a25dab254fdf37a7dff36fd8bbcbd160fd2ba231a50578fc246db94dc5): the new signer's reserve sponsored, signed by the one officer |
| Signer removed, thresholds lowered to 1 | [`3cb759a5…`](https://stellar.expert/explorer/testnet/tx/3cb759a5116851f05f183d3c40962ca264fef8afc88ce92d0424ffb32d035ff3): signed by both officers |

**Offering deployment.** The treasury's officers sign the digest of an offering's terms, and the platform deploys its contracts under those terms in four transactions: token, sale, allowlist grant and activation.

| | |
|---|---|
| Video | [▶ Deploying a tokenized offering on Stellar](https://youtu.be/A3-pImhUwvg) (2:04) |
| Token | [`CD2YEHIT…EIOWQLAM`](https://stellar.expert/explorer/testnet/contract/CD2YEHIT7LIYNYQCJJO5B4JSGLGVHCX2WYRNNCQMH4ESKD44EIOWQLAM), `offer-token` WASM `a2091175…`, deployed by [`5748fcb6…`](https://stellar.expert/explorer/testnet/tx/5748fcb603aaf92eea2095908826ad12bdedbf2ba353d39e636e8cadc6c9ecff) |
| Sale | [`CAWO54S2…VNPFRJPN7`](https://stellar.expert/explorer/testnet/contract/CAWO54S2WVVMLIIRXR4VY2F7JW5SOO4QTA2U5BEAHQVDCDXVNPFRJPN7), `offer-sale` WASM `49edfc27…`, deployed by [`bdaf3a23…`](https://stellar.expert/explorer/testnet/tx/bdaf3a23a2b53a9f1d0891f32a23bdb5bb7c350b9b376f610a3e7a5d2512c1af) |
| Allowlist grant, activation | [`dcdf5f01…`](https://stellar.expert/explorer/testnet/tx/dcdf5f016490300286a794e5ee09642505f5febcdb551b297684b0ae2ab6491e), [`6e210663…`](https://stellar.expert/explorer/testnet/tx/6e210663e4e36e4cbb3499d82f080aa0ee69ff3896322afa9591f6dfa02a1626) |

Both contracts run the WASM of Deliverable 2's release: `stellar contract info hash --id <contract> --network testnet` gives `a2091175…` and `49edfc27…`, the hashes the [Deliverable 2 page](tranche-1-d2.md) lists.

Each account's page on the explorer shows its balances, the sponsor of each reserve, its signers and its full transaction history.

**No private keys at BWB.** For a user, the platform keeps the Stellar address and nothing else. For its own accounts, it keeps the address and the identifier of the key Privy holds. Users sign in the browser through Privy, and the platform signs by asking Privy to sign a hash ([`privySigner`](../../sdk/src/signing.ts)). The [wallet tests](../../sdk/test/wallet.test.ts) pin down which wallet a user gets: *returns the Stellar wallet Privy already holds for the user and creates none*; *creates a Stellar wallet owned by the user when Privy holds none*; *does not take a Stellar wallet the user connected from outside Privy*.

### How to verify

Open any account above on the explorer. The Signers panel lists the keys that control it, and no BWB key controls a user's account. The History tab lists every transaction.

## Criterion 3 — met ✅

> Published transaction hashes demonstrate reserve sponsorship, trustline creation, and fee-bump execution on testnet.

### Evidence

One transaction shows all three. The investor wallet's creation, [`05165b8c…`](https://stellar.expert/explorer/testnet/tx/05165b8cb7eb7088ddd7688feaa195b7c1ac9a74944f78bfba7348f6f695aaa3), is a fee-bump paid by the sponsor around an inner transaction, [`92b46f30…`](https://stellar.expert/explorer/testnet/tx/92b46f308cd0e026b7df90be15ed444ebcaab2ec774bccb1323a3f5f0781e03e), whose source is a channel account. Its operations are the ones `walletOpeningOperations` builds:

| Operation | Source | What it shows |
|---|---|---|
| `begin_sponsoring_future_reserves` | sponsor | Reserve sponsorship opens for the new account |
| `create_account`, starting balance 0 | sponsor | The account exists with no XLM of its own |
| `change_trust` USDC | the investor | Trustline creation, signed by the investor's key |
| `end_sponsoring_future_reserves` | the investor | The investor accepts the sponsorship |

A second investor wallet was created the same way in [`638c6874…`](https://stellar.expert/explorer/testnet/tx/638c687434e10b0d92324b6c55b1b2d795991feed3245b059b241fe4556a4e9a). The treasury's creation and the signer it adds are sponsored the same way, and every signer change and deployment of criterion 2 is fee-bumped by the same sponsor.

RPC keeps transactions for about seven days and testnet resets from time to time, so the record of each transaction above, with its operations, is committed under [`evidence/d1/`](evidence/d1/).

### How to verify

Open the transaction on the explorer. The header names the fee-bump and its fee source, and the operations list shows the sponsorship, the account and the trustline. The account's page shows the sponsor next to its base reserve and its USDC trustline.

## Criterion 4 — met ✅

> Public CI evidence covers duplicate submission, timeout, retry, and rejected-signature scenarios, with every criterion mapped in the public evidence index.

### Evidence

The [SDK workflow](../../.github/workflows/sdk.yml) runs on every pull request that touches the SDK and on every push to `main` ([run on this branch](https://github.com/BWB-Labs/bwb-stellar/actions/runs/40923689913)). The relay is the one path every envelope takes, so it is where each scenario is handled, and [`test/relay.test.ts`](../../sdk/test/relay.test.ts) is where each is tested:

| Scenario | Tests | Code |
|---|---|---|
| Duplicate submission | *sends nothing and reports the outcome when the envelope already landed* | `submitSigned` in [`relay.ts`](../../sdk/src/relay.ts) looks the envelope up before sending it, so a second submission of the same signatures reports the first one's outcome |
| Timeout | *refuses with time bound passed when the network finds the transaction too late*; *refuses with time bound passed once the chain has closed past it without the transaction*; *says the envelope may still land when it has not landed within a minute*; *says the transaction may still land, naming it, when the network stops answering after submission* | every envelope carries a time bound. `awaitFinal` in [`relay.ts`](../../sdk/src/relay.ts) tells an envelope that can no longer land (`timeBoundPassed`) from one that may still land (`submissionFailed`, with its hash) |
| Retry | *reads the sequence again and resubmits when the chain refuses a stale one*; *stops resubmitting after three stale sequences and names the refusal*; *falls back to the public endpoint when the managed one does not answer* ([`network.test.ts`](../../sdk/test/network.test.ts)) | `submitAsPlatform` rebuilds and resubmits on `txBadSeq`, up to three attempts. `connect` in [`network.ts`](../../sdk/src/network.ts) moves to the next RPC endpoint and refuses one that serves another network |
| Rejected signature | *refuses with signing failed, sending nothing, when an officer’s signature is not of the envelope*; *refuses with signing failed, sending nothing, when a platform key does not sign*; *refuses naming the failure when the network rejects the envelope* | `submitSigned` checks each signature against the envelope's hash before anything is sent. [`isSignatureOf`](../../sdk/src/signing.ts) does the same check for a browser signature, tested in [`signing.test.ts`](../../sdk/test/signing.test.ts) |

The rest of the suite covers the builders of criteria 2 and 3: the sponsored wallet opening and trustline ([`wallet.test.ts`](../../sdk/test/wallet.test.ts)), the treasury's creation, signer changes and signer reads ([`treasury.test.ts`](../../sdk/test/treasury.test.ts)), and the offering's addresses, digest and deployment steps ([`offering.test.ts`](../../sdk/test/offering.test.ts)).

This document is the evidence index: each of the four criteria above, with its code, tests, accounts and transactions.

### How to verify

`bun run test`, from `sdk/`, runs the suite. The CI run linked above shows it passing on GitHub's runners.

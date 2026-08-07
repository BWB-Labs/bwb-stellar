# Integration Architecture

How the BWB platform connects to Stellar. The short version: a Convex control plane prepares unsigned transactions and reconciles state, investors sign with non-custodial embedded wallets, and Soroban contracts enforce the rules. The fiat lane runs entirely off Stellar.

---

## System overview

Three lanes, deliberately separated. Value crosses between them at exactly two points: the CCTP bridge, and the reconciliation the control plane performs.

```
                    +--------------------------------------+
                    |   BWB Convex backend (control plane) |
                    |                                      |
                    |  compliance gates                    |
                    |  builds UNSIGNED XDR envelopes       |
                    |  indexes ledger + contract events    |
                    |  reconciles provider vs ledger       |
                    +----+--------------+--------------+---+
                         |              |              |
        reconciles       |              |  prepares    |  indexes
        provider events  |              |  unsigned    |  events
                         v              |  XDR         v
  === LANE 1: FIAT (Base) ===========   |    === LANE 3: STELLAR / SOROBAN =====
  |                                 |   |    |                                 |
  |   Investor pays BRL             |   |    |   Privy embedded wallet         |
  |         |                       |   |    |   (ed25519, non-custodial)      |
  |         v                       |   |    |         | signs XDR            |
  |   Avenia: BRL <-> USDC          |   +--->|         v                      |
  |   conversion + settlement       |        |   Stellar network              |
  |         |                       |        |   +--------------------------+ |
  |         v                       |        |   | kyc-whitelist            | |
  |   settles ON BASE               |        |   | real-estate-token        | |
  |   (never touches Stellar)       |        |   | distribution (scaffold)  | |
  |                                 |        |   +--------------------------+ |
  ===================================        |   DeFindex vaults (yield)      |
            |                                ================================
            | native USDC                                     ^
            v                                                 |
  === LANE 2: CCTP =========================================== |
  |   Circle CCTP: native USDC, Base <-> Stellar             --+
  |   *** GATED: production activation waits on official     |
  |       CCTP availability on Stellar Mainnet ***           |
  ============================================================
```

Read it as: money becomes USDC in lane 1 without Stellar ever being involved, crosses to Stellar as native USDC in lane 2 once that path is officially available, and everything that is a regulated position — issuance, transfer, distribution — happens in lane 3.

---

## Wallets

Investors hold **Privy embedded wallets**: ed25519 keypairs generated for and controlled by the investor, not by BWB. BWB never holds a user key and never holds user funds.

Three Stellar mechanics make an embedded wallet usable by someone with no prior crypto balance:

| Mechanic | What it removes |
|---|---|
| **CAP-33 sponsored base reserves** | The investor does not have to acquire XLM to have a funded account exist. BWB sponsors the base reserve. |
| **Sponsored USDC trustlines** | The investor does not have to fund a trustline reserve before they can receive USDC. BWB sponsors it. |
| **Fee-bump envelopes** | The investor does not have to hold XLM to submit a transaction. BWB wraps the signed inner transaction in a fee-bump envelope and pays the network fee. |

The signing flow is always the same shape. The control plane builds an **unsigned XDR envelope**, the investor's embedded wallet signs it, and BWB submits it — wrapping it in a fee bump when appropriate. BWB composes the intent; the investor authorizes it. A transaction that moves an investor's position cannot be produced without the investor's signature.

---

## Convex backend

The Convex backend is BWB's control plane. Its four responsibilities:

**Compliance gating.** Before any envelope is built, the backend evaluates whether the operation is permitted: is the investor KYC-approved, does the category allow this offering, is the offering open, does the allocation fit. The on-chain gate in `kyc-whitelist` is the enforcement of last resort, not the first check.

**Transaction preparation.** The backend builds unsigned XDR for investor-signed operations. It holds no investor key and cannot sign on an investor's behalf.

**Operator and admin transactions.** Where BWB acts in its own name — approving a KYC entry, minting into a subscribed position, pausing a contract — the backend signs with **BWB operational keys**. These authorize BWB's own operations only. They are never user keys, and the two key sets are never interchangeable.

**Indexing and reconciliation.** The backend indexes Stellar ledger state and Soroban contract events (`kyc_add`, `mint`, `transfer`, `xfer_from`, `burn`, `burn_from`, `paused`, and the governance events — see [02-contracts.md](02-contracts.md)) into queryable form. It then reconciles two independent records: what the fiat provider reports settling, and what the ledger reports happening. A mismatch is surfaced as a break to be investigated, not smoothed over.

---

## Fiat rail

Brazilian investors fund and redeem in BRL. **Avenia** performs the BRL to USDC conversion and the settlement, **on Base**. This rail never touches Stellar.

```
  Investor pays BRL
        |
        v
  Avenia: BRL <-> USDC conversion and settlement, on Base
        |
        +--> settlement event --> Convex control plane
        |                          reconciled against ledger state
        v
  USDC on Base
```

Two consequences worth being explicit about. First, no Soroban contract in this repository knows anything about BRL, about the fiat provider, or about the Base-side intermediate stablecoin — the contracts only ever see Stellar assets and Stellar addresses. Second, the link between a BRL payment and an on-chain position is made by the control plane's reconciliation, which is exactly why that reconciliation is a first-class responsibility rather than a background job.

---

## CCTP

**Circle CCTP** moves **native USDC between Base and Stellar**. Native matters here: the USDC that lands on Stellar is the real thing, not a wrapped or bridged representation with its own issuer risk, so an offering settling in USDC on Stellar is settling in the same asset the investor funded.

**Production activation is gated on official CCTP availability on Stellar Mainnet.** The architecture assumes this path; it is not switched on, and BWB will not route production value through it before it is officially available.

---

## DeFindex

Yield strategies run through **DeFindex vaults**, one segregated vault per strategy. Segregation is the point: each strategy's capital, exposure, and return are separately observable rather than pooled into a single operational balance, which is what makes per-strategy reporting to investors and regulators tractable.

---

## SDK modules

The TypeScript SDK, published as **`@bwb/stellar-sdk`**, sits between the Convex backend and the Soroban contracts. Source in `sdk/src/`.

| Module | Status | Contents |
|---|---|---|
| `client.ts` | **Implemented** | `BWBStellarClient` — constructs Horizon and Soroban RPC clients with per-network defaults and the right network passphrase, `getAccountBalance(publicKey)` reads the native balance from Horizon, `getTransactionStatus(hash)` reads a transaction from Soroban RPC. |
| `kyc.ts` | **Stub** | `KYCWhitelistClient` shape and types are defined; `isApproved`, `addToWhitelist`, and `removeFromWhitelist` throw. |
| `token.ts` | **Stub** | `RealEstateTokenClient` shape and types are defined; `getBalance`, `getTotalSupply`, `getOffering`, and `mint` throw. |

The intended split, once the contract modules are built out, is that the SDK produces unsigned XDR and the caller decides who signs it: the investor's embedded wallet for investor operations, a BWB operational key for operator and admin operations. The SDK itself holds no keys.

---

## Environment configuration

| Variable | Description |
|---|---|
| `STELLAR_NETWORK` | `testnet` or `mainnet` |
| `SOROBAN_RPC_URL` | Soroban RPC endpoint; overrides the per-network default |
| `HORIZON_URL` | Horizon API endpoint; overrides the per-network default |
| `KYC_CONTRACT_ID` | Deployed `kyc-whitelist` contract address |
| `OPERATOR_PUBLIC_KEY` | Stellar public key of the BWB operational key (ed25519) |

`OPERATOR_PUBLIC_KEY` is a **public** key — it is used to identify and verify, never to sign. No private key is stored in this repository, in its environment configuration, or in any file it produces. Investor keys live in Privy embedded wallets under investor control; BWB operational private keys live in the private backend's secret management.

---

## Repository split

```
bwb-stellar  (this repository, public, Apache-2.0)
  contracts/   kyc-whitelist, real-estate-token, distribution
  sdk/         @bwb/stellar-sdk
  scripts/     deployment and funding scripts
  docs/        this documentation

bwb product repository  (private)
```

The private repository depends on the public one, never the reverse. The Soroban contracts have no dependency on BWB's backend and can be deployed and used independently by any platform with a comparable regulatory model.

The private repository holds a **provider abstraction over payment and conversion rails**: a common interface for quoting, initiating, and confirming a conversion, with one implementation per rail, so the application code that subscribes an investor does not change when a rail is added, swapped, or run in parallel with another. Provider credentials and provider-specific webhook handling live behind that interface, entirely in the private repository. Nothing in this public repository names or depends on any specific provider.

---

## Distribution attribution

Distribution attribution and commissioning are computed in off-chain BWB ledgers fed by Stellar transactions and Soroban contract events.

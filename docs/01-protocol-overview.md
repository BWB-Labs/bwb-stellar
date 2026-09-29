# Protocol Overview

## What BWB does

BWB Digital Assets is a regulated real-estate investment platform in Brazil, operating since 2023 under **CVM Resolution 88** — the Brazilian securities rule that governs crowdfunded and tokenized offerings. BWB structures real-estate debt and equity offerings, onboards and classifies investors, and administers the full lifecycle of each offering from subscription through maturity.

This repository is the Stellar side of that platform: the Soroban contracts, the TypeScript SDK, and the deployment scripts that make Stellar BWB's primary network rail.

---

## Why Stellar

Stellar is the **execution layer** for BWB's regulated offerings: issuance, transfers, and distributions all settle on Stellar, and the compliance rules that constrain them are enforced by Soroban contracts rather than by application code alone.

Three properties drive that choice. Settlement finality is fast and cheap enough that a quarterly distribution to a few hundred holders is an ordinary operation rather than a budget line. USDC is a first-class Stellar asset, which gives every offering a single settlement unit that investors can hold directly. And Soroban lets the eligibility and supply constraints that CVM 88 imposes live on-chain, where they are auditable by anyone rather than asserted by BWB.

---

## Control plane and execution layer

BWB runs a Convex backend that acts as the **control plane**. It never becomes a custodian and never becomes a second source of truth about balances. Its job is:

- **Compliance gating** — deciding whether a given investor, at a given moment, may subscribe to or transfer a given position, based on KYC state, category, and offering rules.
- **Transaction preparation** — building **unsigned XDR transaction envelopes** for the investor to sign. The control plane composes the operations; it does not hold the key that authorizes them.
- **Indexing** — reading Stellar ledger state and Soroban contract events into queryable form so the product surface can show positions, history, and offering status.
- **Reconciliation** — matching provider events (payment, conversion, settlement) against ledger events, and surfacing breaks rather than silently papering over them.

Stellar and Soroban are the **execution layer**. State that matters — who holds what, what the offering allows, whether transfers are permitted right now — lives on the ledger and in contract storage.

The system is **non-custodial**. Investors sign with Privy embedded wallets holding ed25519 keys. BWB never holds user keys and never holds user funds. Where BWB must act in its own name — approving a KYC entry, minting into a subscribed position — it signs with its own operational keys, which authorize BWB's operations and nothing else.

---

## The four integration surfaces

### 1. Privy embedded non-custodial wallets

Each investor gets an embedded Stellar wallet whose ed25519 key is under the investor's control. Three Stellar mechanics make this usable for people who have never held a crypto asset:

- **CAP-33 sponsored base reserves** — BWB sponsors the account's base reserve, so an investor does not need to pre-fund an account with XLM before they can hold anything.
- **Sponsored USDC trustlines** — the trustline reserve is sponsored the same way, so receiving USDC requires no prior balance.
- **Fee-bump envelopes** — BWB wraps the investor's signed transaction in a fee-bump envelope and pays the network fee, so the investor never needs XLM to transact.

The investor signs XDR prepared by the control plane. The signature is theirs; the fee and the reserves are BWB's.

### 2. Soroban regulated offering contracts (this repository)

The offering contracts hold the position record for **debt and equity offerings** and enforce the constraints that make the position a compliant instrument rather than a bearer token:

- **Eligibility enforcement** — a transfer or issuance to an address that is not KYC-approved is rejected by the contract, not by the UI.
- **Allocation enforcement** — total issued supply is capped at the amount authorized in the offering's immutable metadata.
- **Lifecycle controls** — pause, cooling-off, cancellation, and refund. Of these, **pause is implemented today**; cooling-off, cancellation, and refund are named in the target architecture and not yet implemented. See [02-contracts.md](02-contracts.md) for exactly what exists in source.
- **Auditable events** — every state change that matters emits a contract event, giving anyone a reconstructable history without trusting BWB's database.
- **No investor PII on-chain** — the per-investor record is the wallet address, the investor category, the approval timestamp, and the approving key. Names, documents, and identity data stay off-chain. (Offering-level data such as the property address is public by design; it describes the asset, not a person.)

### 3. DeFindex vaults

Idle offering capital and yield strategies are held in **DeFindex vaults**, one segregated vault per strategy, so strategy exposure is explicit and separable rather than pooled into a single operational balance.

### 4. Circle CCTP

**CCTP** carries native USDC between Base and Stellar, so USDC that arrives on the Base side can move to Stellar as native USDC rather than as a wrapped representation. Circle CCTP has been live on Stellar Mainnet since 19 May 2026. BWB builds the bidirectional route on testnet in Tranche 2 and activates it on mainnet in Tranche 3, using Circle's published contracts and the mandatory CctpForwarder flow for Stellar recipients.

---

## The fiat rail

Brazilian investors fund in BRL. **Avenia** handles BRL to USDC conversion and settlement, and it does so **on Base**. That rail never touches Stellar.

The separation is deliberate and worth stating plainly: the fiat lane converts and settles on Base, the CCTP lane moves native USDC from Base to Stellar, and the Stellar lane does issuance, transfer, and distribution. The control plane reconciles Avenia's settlement events against ledger state, which is what ties a BRL payment to an on-chain position. No Soroban contract in this repository has any knowledge of the Base-side intermediate stablecoin or of the fiat provider.

---

## The CVM 88 compliance model

CVM Resolution 88 constrains who may hold a tokenized security, how much may be issued, and what must be on the record. The contracts implement that as follows.

**Investor eligibility categories.** Every approved investor carries one of three categories, matching the regulatory classification:

| Category | Meaning under CVM 88 |
|---|---|
| `Retail` | Standard retail investor |
| `Qualified` | Investor with R$1M or more in financial assets |
| `Professional` | Institutional, or R$10M or more in financial assets |

The category is stored on-chain with the approval; the evidence that justifies it stays off-chain in BWB's compliance records.

**Transfer restriction via the KYC gate.** The offering token does not allow a transfer unless both the sender and the recipient are currently approved in the KYC contract. The check is a cross-contract call made inside the token contract, so it cannot be bypassed by calling the contract directly, by using a different client, or by routing around the product surface. Revocation takes effect immediately: an investor removed from the whitelist can no longer send or receive in the very next ledger.

**Immutable offering metadata.** Each offering contract stores its own metadata at initialization and never mutates it: the offering identifier, the property address, the total raise, the target IRR, the maturity date, the **CVM authorization code**, and the **`max_supply` cap**. The cap is enforced on every issuance — the contract rejects any mint that would push total supply past the amount CVM authorized. The authorization code being on-chain and immutable means the regulatory basis for the offering is verifiable by anyone reading the ledger.

---

## Roles

| Role | Key | Authority |
|---|---|---|
| **Admin** | BWB cold wallet | Governance: two-step admin handover, set or replace the operator, pause and unpause, point the token at a different KYC contract |
| **Operator** | BWB operational hot key | Day-to-day operations: approve and revoke KYC entries, mint into subscribed positions |
| **Investor** | Non-custodial embedded wallet | Hold, transfer to other approved investors, approve allowances, burn |

Admin and Operator are separated so that routine operations never require the cold key. Admin handover is two-step — the current admin proposes a successor, and the successor must accept — so a typo in an address cannot strand the contract with an unreachable admin.

---

## Distribution attribution

Distribution attribution and commissioning are computed in **off-chain BWB ledgers fed by Stellar transactions and Soroban contract events** — the chain records the payment, and BWB's books attribute it.

---

## Current status

Nothing in this repository is deployed to testnet or mainnet.

| Component | Status |
|---|---|
| `kyc-whitelist` contract | Implemented; 16 tests in source |
| `real-estate-token` contract | Implemented, SEP-0041 surface plus BWB extensions; 31 tests in source |
| `distribution` contract | **Scaffold — Tranche 3 deliverable.** Initialization and getters exist; the distribution entrypoint intentionally panics. 3 tests in source |
| TypeScript SDK (`@bwb/stellar-sdk`) | `client.ts` is functional (RPC and Horizon clients, account balance, transaction status). The contract modules are stubs that throw |
| Testnet deployment | Not deployed |
| Mainnet deployment | Not deployed |

Test counts above are counts of test functions present in source, not a claim about a passing run. CI is currently red.

---

## Open source

The Soroban contracts and the SDK in this repository are licensed **Apache 2.0**. Any regulated platform that needs an on-chain eligibility gate plus a supply-capped, metadata-bearing security token can build on this directly.

BWB's product code — the investor-facing application and the Convex control plane — lives in a separate private repository and is not part of this one.

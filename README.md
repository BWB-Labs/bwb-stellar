# BWB on Stellar

> Soroban contracts and TypeScript SDK for BWB Digital Assets' Stellar integration.

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

---

## What's in this repo

| Component | Status |
|---|---|
| `contracts/kyc-whitelist` | Implemented — on-chain investor eligibility registry (16 unit tests) |
| `contracts/real-estate-token` | Implemented — SEP-0041 offering token with eligibility gate and `max_supply` cap (31 unit tests) |
| `contracts/distribution` | Scaffold — Tranche 3 deliverable; entrypoint intentionally panics until implemented (3 unit tests) |
| `sdk/` | TypeScript SDK — `client.ts` (RPC/Horizon clients) functional; contract bindings are stubs for later tranches |
| `scripts/` | Testnet and mainnet deploy scripts (atomic deploy + initialize) |

Nothing is deployed to testnet or mainnet yet.

## How it fits BWB's platform

BWB Digital Assets is a regulated real-estate investment platform in Brazil (CVM Resolution 88, operating since 2023) that connects real-estate issuers, qualified investors, and distribution partners. This project makes Stellar the platform's primary network rail.

BWB's backend (Convex) acts as the orchestration and control plane: it gates compliance, prepares **unsigned XDR transaction envelopes**, indexes network state, and reconciles provider and ledger events. Stellar and Soroban form the execution layer. Investors sign their own transactions with **Privy embedded non-custodial wallets** — BWB never holds user keys or funds.

The integration has four Stellar surfaces:

1. **Privy embedded wallets** — non-custodial ed25519 accounts with sponsored base reserves (CAP-33), sponsored USDC trustlines, and fee-bump transaction envelopes.
2. **Soroban regulated offering contracts** (this repo) — debt and equity positions with eligibility and allocation enforcement, pause / cooling-off / cancellation / refund controls, and auditable events. No PII on-chain.
3. **DeFindex vaults** — segregated yield strategies on Soroban.
4. **Circle CCTP** — native-USDC transport between Base and Stellar; production activation is gated on official CCTP availability on Stellar Mainnet.

BRL entry and exit is handled by Avenia (BRL ↔ USDC conversion, settling on Base) — connective tissue that never touches Stellar. Distribution attribution and commissioning are off-chain BWB ledgers fed by Stellar transactions and Soroban contract events.

## Contracts

| Contract | Purpose | Status | Unit tests |
|---|---|---|---|
| `kyc-whitelist` | Investor eligibility registry with admin/operator roles and two-step admin transfer | Implemented | 16 |
| `real-estate-token` | SEP-0041 offering token; every transfer checks eligibility via `kyc-whitelist`; mint capped by `max_supply` | Implemented | 31 |
| `distribution` | Pro-rata payout of the offering's payout asset (USDC) to token holders | Scaffold (Tranche 3) | 3 |

See [docs/02-contracts.md](docs/02-contracts.md) for full specifications.

## Repository layout

```
bwb-stellar/
├── contracts/
│   ├── kyc-whitelist/       # On-chain eligibility registry (CVM Res. 88 gate)
│   ├── real-estate-token/   # SEP-0041 offering token
│   └── distribution/        # Payout scaffold (Tranche 3)
├── sdk/                     # TypeScript SDK (@bwb/stellar-sdk)
├── scripts/                 # Testnet + mainnet deploy scripts
├── docs/                    # Technical documentation
└── audit/                   # Audit reports (planned; see audit/)
```

## Build & test

```bash
git clone https://github.com/techlibs/bwb-stellar.git
cd bwb-stellar

# Contract tests
cargo test -p kyc-whitelist
cargo test -p real-estate-token
cargo test -p distribution

# SDK (client.ts functional; contract bindings are stubs)
cd sdk && npm install && npm test
```

## Documentation

| Document | Contents |
|---|---|
| [docs/01-protocol-overview.md](docs/01-protocol-overview.md) | Protocol overview — control plane vs execution layer, compliance model, roles |
| [docs/02-contracts.md](docs/02-contracts.md) | Contract specifications — functions, storage, events, invariants |
| [docs/03-integration-architecture.md](docs/03-integration-architecture.md) | Integration architecture — wallets, fiat rail, CCTP, DeFindex, SDK |

## Future work

Planned for later tranches: `distribution` contract implementation (Tranche 3) and full SDK contract bindings.

## License

Everything in this repository — contracts, SDK, scripts, docs — is licensed [Apache 2.0](LICENSE) and will remain open. The BWB product application (frontend, Convex backend, EVM contracts) is proprietary and lives in a private repository.

## Contact

- **Website:** [bwbi.com.br](https://bwbi.com.br)
- **Platform:** [app.bwbi.com.br](https://app.bwbi.com.br)
- **Email:** contato@bwbi.com.br

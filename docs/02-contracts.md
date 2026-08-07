# Soroban Contracts

Three Soroban contracts, all Rust, all Apache-2.0:

| Contract | Path | Role |
|---|---|---|
| `kyc-whitelist` | `contracts/kyc-whitelist/src/lib.rs` | On-chain eligibility registry — the CVM 88 gate |
| `real-estate-token` | `contracts/real-estate-token/src/lib.rs` | Offering position token — SEP-0041 surface plus BWB extensions |
| `distribution` | `contracts/distribution/src/lib.rs` | Yield distribution — **scaffold, Tranche 3 deliverable** |

This document describes what is in source today. Nothing here is deployed to testnet or mainnet.

All three contracts share the same TTL constants, sized for Stellar mainnet at roughly five seconds per ledger close: one day is 17,280 ledgers, instance storage is bumped to 60 days whenever it has under 30 days left, and persistent storage is bumped to 120 days whenever it has under 30 days left. Investor data is deliberately the longest-lived tier.

---

## 1. `kyc-whitelist`

The eligibility registry. An address is either in it or not; the token contract asks this contract before it moves anything.

**Tests in source: 16.**

### Storage layout

| `DataKey` variant | Value type | Tier | Purpose |
|---|---|---|---|
| `Admin` | `Address` | Instance | Cold wallet, governance only |
| `PendingAdmin` | `Address` | Instance | Successor during a two-step handover; removed on accept |
| `Operator` | `Address` | Instance | Operational hot key; absent unless explicitly set |
| `Entry(Address)` | `WhitelistEntry` | Persistent | One approval record per investor |

Approval records go in persistent storage so they survive independently of the contract instance; governance config goes in instance storage.

### Public types

```rust
pub enum InvestorCategory {
    Retail,       // CVM 88: standard retail investor
    Qualified,    // CVM 88: R$1M or more in financial assets
    Professional, // CVM 88: institutional, or R$10M or more
}

pub struct WhitelistEntry {
    pub investor_category: InvestorCategory,
    pub approved_at: u64,     // ledger timestamp at approval
    pub approved_by: Address, // the admin or operator that approved
}
```

Note what is **not** here: no name, no document number, no identity data. The on-chain record is an address, a category, a timestamp, and the approving key.

### Functions

| Function | Auth required | Behavior |
|---|---|---|
| `initialize(admin)` | `admin` | Sets the admin. Panics `"Already initialized"` if the admin key is already present. No operator is set here. |
| `propose_admin(new_admin)` | current `admin` | Step 1 of handover. Writes `PendingAdmin`. Emits `adm_prop`. |
| `accept_admin()` | pending admin | Step 2. Promotes `PendingAdmin` to `Admin` and clears the pending slot. Panics `"No pending admin proposal"` if none. Emits `adm_new`. |
| `set_operator(operator)` | `admin` | Sets or replaces the operator. Emits `op_set`. |
| `remove_operator()` | `admin` | Deletes the operator key; afterwards only the admin can run KYC operations. Emits no event. |
| `add(caller, address, category)` | `caller`, and `caller` must be admin or operator | Writes the `WhitelistEntry` with the current ledger timestamp and `caller` as approver, then extends the entry TTL. Emits `kyc_add`. |
| `remove(caller, address)` | `caller`, and `caller` must be admin or operator | Panics `"Address not in whitelist"` if absent, otherwise deletes the entry. Emits `kyc_rm`. |
| `is_ok(address) -> bool` | none | Presence check. This is the function `real-estate-token` calls cross-contract. |
| `get_entry(address) -> Option<WhitelistEntry>` | none | Full record, or `None`. |
| `get_admin() -> Address` | none | Current admin. |
| `get_operator() -> Option<Address>` | none | Current operator, or `None` when unset or removed. |
| `extend_ttl()` | none | Bumps instance storage. Intended as a periodic heartbeat. |
| `extend_entry_ttl(address)` | none | Bumps one investor entry. Panics `"Address not in whitelist"` if absent. |

Authorization is two-layered on `add` and `remove`: `caller.require_auth()` proves the caller signed, and `require_admin_or_operator` proves the caller is entitled. Both must hold, and failure of the second panics with `"Unauthorized: caller is not admin or operator"`.

### Events emitted

| Symbol | Payload | Emitted by |
|---|---|---|
| `kyc_add` | `address` | `add` |
| `kyc_rm` | `address` | `remove` |
| `adm_prop` | `new_admin` | `propose_admin` |
| `adm_new` | `new_admin` | `accept_admin` |
| `op_set` | `operator` | `set_operator` |

`remove_operator` deliberately emits nothing today.

### Invariants

- Admin is set at initialization and always set thereafter; `initialize` cannot run twice.
- Operator is optional. With no operator, only the admin can write to the whitelist.
- Admin handover is never single-step: the successor must sign `accept_admin` before it takes effect, so a wrong address cannot strand the contract.
- `is_ok` takes no auth and mutates nothing, so any contract can gate on it.
- Removing an investor deletes the entry but not the event history — the audit trail of who was approved, by whom, and when, remains reconstructable from events.

---

## 2. `real-estate-token`

One token contract per offering. Implements the SEP-0041 token surface, and adds the CVM 88 constraints: an eligibility gate on every movement, an authorized supply cap, immutable offering metadata, and an emergency pause.

**Tests in source: 31**, including cross-contract tests that register a real `kyc-whitelist` alongside the token.

### Storage layout

| `DataKey` variant | Value type | Tier | Purpose |
|---|---|---|---|
| `Admin` | `Address` | Instance | Cold wallet |
| `PendingAdmin` | `Address` | Instance | Successor during handover |
| `Operator` | `Address` | Instance | Operational hot key; set at initialization |
| `KycContract` | `Address` | Instance | Address of the eligibility registry to consult |
| `TotalSupply` | `i128` | Instance | Issued supply in smallest units |
| `Name` | `String` | Instance | Token name |
| `Symbol` | `String` | Instance | Token symbol |
| `Metadata` | `OfferingMetadata` | Instance | Immutable offering record |
| `Paused` | `bool` | Instance | Emergency stop |
| `Balance(Address)` | `i128` | Persistent | Per-investor balance |
| `Allowance(AllowanceKey)` | `AllowanceValue` | Temporary | Spend authorizations, TTL matched to expiry |

`DECIMALS` is a compile-time constant of `7`, matching the Stellar native asset convention.

### Public types

```rust
pub struct OfferingMetadata {
    pub offering_id: String,       // BWB internal offering identifier
    pub property_address: String,  // property address
    pub total_raise: i128,         // total raise, in BRL cents
    pub max_supply: i128,          // SC-H05: token units authorized under CVM 88
    pub target_irr_bps: u32,       // target IRR in basis points (2080 = 20.80%)
    pub maturity_date: u64,        // Unix timestamp of expected maturity
    pub cvm_authorization: String, // CVM Resolution 88 authorization code
}

pub struct AllowanceKey {
    pub from: Address,
    pub spender: Address,
}

pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32, // absolute ledger number at which it expires
}
```

`max_supply` was added as security fix **SC-H05**. Without it, the offering's regulatory authorization was a number written in metadata that nothing enforced.

### Functions

**Initialization and governance**

| Function | Auth required | Behavior |
|---|---|---|
| `initialize(admin, operator, kyc_contract, name, symbol, metadata)` | `admin` | Writes all instance keys, sets supply to 0 and paused to false. Panics `"Already initialized"` on a second call. |
| `propose_admin(new_admin)` | `admin` | Step 1 of handover. Emits `adm_prop` (SC-L01). |
| `accept_admin()` | pending admin | Step 2. Panics `"No pending admin proposal"` if none pending. Emits `adm_new` (SC-L01). |
| `set_operator(operator)` | `admin` | Replaces the operator. Emits `op_set` (SC-L01). |
| `set_kyc_contract(new_kyc_contract)` | `admin` | **SC-H04.** Repoints the eligibility gate without redeploying the token. Before committing, **SC-X04** probes the candidate by invoking `is_ok` on it, so a wrong ABI or an uninitialized contract traps here instead of silently disabling the gate. Emits `kyc_upd` with `(old, new)`. |
| `pause()` | `admin` | Sets `Paused`. Emits `paused`. |
| `unpause()` | `admin` | Clears `Paused`. Emits `unpaused`. |

`set_kyc_contract` is powerful and the source says so: the new registry must already contain every current investor, because the very next transfer or mint uses it.

**Issuance**

| Function | Auth required | Behavior |
|---|---|---|
| `mint(caller, to, amount)` | `caller`, and `caller` must be admin or operator | Asserts `amount > 0`, asserts not paused (**SC-X03**), checks `to` is KYC-approved, then asserts `total_supply + amount <= metadata.max_supply` (**SC-H05**) before crediting. Extends the recipient balance TTL. Emits `mint` with `(to, amount)`. |

**SEP-0041 surface**

| Function | Auth required | Behavior |
|---|---|---|
| `balance(id) -> i128` | none | Balance, `0` when absent. |
| `transfer(from, to, amount)` | `from` | Asserts `amount > 0`, not paused, and that **both `from` and `to`** are KYC-approved (**SC-H01** added the sender-side check). Emits `transfer` with `(from, to, amount)`. |
| `transfer_from(spender, from, to, amount)` | `spender` | Same asserts and the same two-sided KYC check on `from` and `to`; the spender itself is not KYC-checked. Consumes allowance, then moves the balance. Emits `xfer_from` with `(spender, from, to, amount)`. |
| `approve(from, spender, amount, expiration_ledger)` | `from` | Asserts not paused (**SC-X07** — a freeze must also stop new approvals), asserts `expiration_ledger > current sequence` strictly (**SC-H03**, since `>=` would permit a zero-TTL entry), and `amount >= 0`. An amount of `0` deletes the allowance; otherwise the temporary entry's TTL is set to match the expiry exactly. Emits `approve` with `(from, spender, amount, expiration_ledger)`. |
| `allowance(from, spender) -> i128` | none | Returns the stored amount only while `expiration_ledger >= current sequence`; otherwise `0`. |
| `burn(from, amount)` | `from` | Asserts `amount > 0` and not paused. **No KYC check** — a holder can always exit their position. Emits `burn` with `(from, amount)`. |
| `burn_from(spender, from, amount)` | `spender` | Same, via allowance. Also no KYC check. Emits `burn_from` with `(spender, from, amount)`. |
| `decimals() -> u32` | none | Constant `7`. |
| `name() -> String` | none | Stored name. |
| `symbol() -> String` | none | Stored symbol. |

**BWB reads and TTL**

| Function | Auth required | Behavior |
|---|---|---|
| `total_supply() -> i128` | none | Issued supply, `0` when unset. |
| `get_offering() -> OfferingMetadata` | none | The immutable offering record. |
| `get_admin() -> Address` | none | Current admin. |
| `get_operator() -> Address` | none | Current operator. Returns `Address`, not `Option<Address>` — the operator is mandatory at initialization for this contract, unlike in `kyc-whitelist`. |
| `is_paused() -> bool` | none | Pause state. |
| `nav() -> i128` | none | `total_raise * 10^DECIMALS / total_supply`, returning `0` while supply is zero. Since `total_supply` counts smallest units, the `10^7` scalar converts per-unit to per-whole-token: the result is **BRL cents per whole token**. |
| `extend_ttl()` | none | Bumps instance storage. |
| `extend_balance_ttl(investor)` | none | Bumps one balance entry. Panics `"No balance for address"` if absent. |

### Events emitted

| Symbol | Payload | Emitted by |
|---|---|---|
| `mint` | `(to, amount)` | `mint` |
| `transfer` | `(from, to, amount)` | `transfer` |
| `xfer_from` | `(spender, from, to, amount)` | `transfer_from` |
| `approve` | `(from, spender, amount, expiration_ledger)` | `approve` |
| `burn` | `(from, amount)` | `burn` |
| `burn_from` | `(spender, from, amount)` | `burn_from` |
| `kyc_upd` | `(old_kyc_contract, new_kyc_contract)` | `set_kyc_contract` |
| `paused` | `()` | `pause` |
| `unpaused` | `()` | `unpause` |
| `adm_prop` | `new_admin` | `propose_admin` |
| `adm_new` | `new_admin` | `accept_admin` |
| `op_set` | `operator` | `set_operator` |

This event set is what the control plane indexes; between `mint`, `transfer`, `xfer_from`, `burn`, and `burn_from`, every supply and balance change is reconstructable from the ledger alone.

### Invariants

- `total_supply` equals the sum of all balances. Every credit and debit path updates both sides.
- `total_supply` never exceeds `metadata.max_supply` (SC-H05). Minting exactly to the cap succeeds; one unit past it panics `"Mint exceeds CVM-88 authorized offering cap"`.
- No balance goes negative — `"Insufficient balance"` fires before any deduction.
- Both parties to a transfer must be KYC-approved at the moment of transfer, so revoking an investor blocks them immediately, including on allowances granted before the revocation.
- Burns are exempt from the KYC gate by design.
- `metadata`, `name`, and `symbol` are written once at initialization and have no setter.
- `decimals()` is always `7`.
- A self-transfer is a no-op (**SC-X01**). Without the early return, the read-modify-write of the sender and recipient balances would overwrite the debit with the credit and inflate the balance.
- A transfer bumps the recipient's balance TTL, and also the sender's when a remainder is left (**SC-H02**). Without the sender-side bump, a partial transfer could let the sender's remaining balance expire out of storage.
- Allowances live in temporary storage with a TTL matched to `expiration_ledger`, so they cannot outlive their own expiry.
- Pause blocks minting, transfers, `transfer_from`, burns, and new approvals — but never touches stored balances.

---

## 3. `distribution`

**Status: scaffold. Tranche 3 deliverable.** Initialization, its guard, the getters, and the event type exist. The distribution entrypoint intentionally panics.

**Tests in source: 3.**

### What exists today

**Storage layout**

| `DataKey` variant | Value type | Tier | Purpose |
|---|---|---|---|
| `Admin` | `Address` | Instance | Distribution admin |
| `TokenContract` | `Address` | Instance | The offering token to distribute against |
| `KycContract` | `Address` | Instance | Eligibility registry for holder enumeration |
| `PayoutAsset` | `Address` | Instance | **SC-C03.** The payout asset, fixed at initialization |
| `DistributionCount` | `u64` | Instance | Completed distributions |
| `Distribution(u64)` | — | Instance | Declared for per-distribution records; not yet written or read |

The payout asset is **USDC on Stellar**.

**Public type**

```rust
pub struct DistributionEvent {
    pub distribution_id: u64,
    pub total_amount: i128,
    pub per_token_amount: i128,
    pub holder_count: u32,
    pub executed_at: u64,
    pub asset: Address, // always the registered payout asset, never caller-supplied
}
```

**Functions**

| Function | Auth required | Behavior |
|---|---|---|
| `initialize(admin, token_contract, kyc_contract, payout_asset)` | `admin` | **SC-C01.** Panics `"Already initialized"` if the admin key exists. This guard is load-bearing: `require_auth()` only proves that the *supplied* address signed, so without the check any caller could re-run initialization with their own address and take over the contract. |
| `trigger_distribution(total_amount)` | none reached | **SC-C02.** Panics `"trigger_distribution: not yet implemented — Tranche 3 deliverable"` on entry. The panic is deliberate: it stops the contract from emitting distribution events that no transfer backs. |
| `distribution_count() -> u64` | none | Completed distributions, `0` when unset. |
| `get_admin() -> Option<Address>` | none | Admin, or `None` before initialization. |
| `get_payout_asset() -> Option<Address>` | none | Registered payout asset, or `None` before initialization. |

There is no getter for `TokenContract` or `KycContract` yet. `initialize` bumps instance TTL; there is no persistent tier in this contract.

The three tests cover initialization writing the expected state, the SC-C01 re-initialization guard panicking, and the SC-C02 scaffold guard panicking.

### Planned behavior (Tranche 3)

A distribution will pay USDC pro rata to token holders: enumerate KYC-approved holders, read each balance from the token contract, compute each share as `total_amount * balance / total_supply`, transfer USDC from the contract to each holder, and only after every transfer succeeds emit `DistributionEvent` and increment `DistributionCount`.

Two design decisions are already fixed in the scaffold. The payout asset is never a call parameter — it is read from `DataKey::PayoutAsset` (SC-C03), so a caller cannot redirect a distribution into an asset of their choosing. And the event is emitted last, after transfers, so an event on-chain always means the money actually moved.

---

## Cross-contract interaction

The token contract calls the whitelist contract directly through `env.invoke_contract`. Nothing between the caller and the ledger can skip it.

```
   Operator (BWB operational key)
        |
        |-- kyc-whitelist::add(caller, investor, category)
        |
        '-- real-estate-token::mint(caller, investor, amount)
                   |
                   '--> kyc-whitelist::is_ok(investor)
                             true  -> cap checked, then minted
                             false -> panic, whole transaction reverts

   Investor (embedded non-custodial wallet)
        |
        '-- real-estate-token::transfer(from, to, amount)
                   |
                   |--> kyc-whitelist::is_ok(from)   <- SC-H01
                   '--> kyc-whitelist::is_ok(to)
                             either false -> panic, whole transaction reverts
```

Because the check happens inside the token contract, calling the token directly with any client produces the same rejection as calling it through BWB's product surface. The gate is a property of the asset, not of the application.

`set_kyc_contract` is the only way to change which registry answers, it is admin-only, and it probes the candidate for a working `is_ok` before committing (SC-X04).

The `distribution` contract stores the token and registry addresses at initialization but does not call them yet.

---

## Security notes

**Two-step admin handover.** Both `kyc-whitelist` and `real-estate-token` require `propose_admin` followed by `accept_admin` signed by the successor. A single-step transfer to a mistyped address would permanently strand the contract.

**Re-initialization guards.** All three contracts check for an existing admin key and panic rather than overwrite. In `distribution` this is tracked as SC-C01, with the reasoning spelled out in source: `require_auth()` authenticates the address you pass in, so an unguarded `initialize` is an open takeover.

**Interface probe before repointing the gate (SC-X04).** `set_kyc_contract` invokes `is_ok` on the candidate contract before storing it, so a wrong address traps at the governance call rather than at the next investor transfer.

**Pause covers issuance and approvals, not just transfers.** SC-X03 extended the pause check to `mint` and SC-X07 to `approve`. A pause that blocked transfers but let new supply or new spend authorizations through would not be a freeze.

**Storage tiers chosen against silent expiry.** Balances and whitelist entries are persistent and bumped to 120 days on every write; allowances are temporary with a TTL matched to their declared expiry; governance config is instance-tier. SC-H02 added the sender-side TTL bump on partial transfers, which closes the case where a sender's remainder could quietly expire.

**Self-transfer no-op (SC-X01).** Guarded explicitly, since the read-modify-write sequence would otherwise mint value out of nothing.

**Supply cap enforced on-chain (SC-H05).** The CVM-authorized maximum is not just recorded, it is checked on every mint.

**Immediate revocation.** Because the eligibility check is read at transfer time rather than cached, removing an investor from the whitelist blocks them in the next ledger, including through allowances granted earlier.

**No formal external audit has been performed.** The `SC-` identifiers throughout refer to BWB's internal security review findings and their fixes, not to a third-party audit report.

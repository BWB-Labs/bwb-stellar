# offer-sale

Escrow and lifecycle of one offering, one instance per offering. It is a port of Base's `OfferTokenSale`: investors reserve tokens by paying USDC, and the issuer finalizes or the offering fails. Tokens are then released, or payments are refunded. Investors may withdraw during their cooling-off window.

**Draft (interface v0.1).** The contract lands in L4, which updates this document. The cross-cutting rules are in [interface.md](interface.md).

## States

```
Preparing ──activate──▶ Active ──finalize──▶ Successful
                          │
                          ├──mark_failed──▶ Failed
                          └──cancel───────▶ Failed
```

Every transition emits `state_changed`. No state goes back.

## Constructor

| Argument | Meaning |
|---|---|
| `owner` | Issuer's treasury |
| `pauser` | Platform multisig |
| `upgrader` | Platform multisig |
| `token` | The offering's `offer-token` |
| `payment_asset` | The USDC contract |
| `allowlist` | The global `offer-allowlist` |
| `price` | Stroops per token |
| `total_tokens` | Tokens for sale: the token's whole supply |
| `hard_cap` | Maximum raised, in stroops. At least one token's price. |
| `cooling_off_secs` | Cooling-off window. At least 1 day; 5 days in Base's offerings. |
| `min_success_percent` | Share of `total_tokens` that must sell to finalize. At least 10; 75 in Base's offerings. |

The minimums are the ones Base's factory enforces.

## Reservation

One persistent entry per investor:

| Field | Meaning |
|---|---|
| `schema` | Struct version |
| `tokens` | Tokens reserved |
| `paid` | Stroops paid |
| `last_purchase_at` | Ledger time of the last purchase. The cooling-off window counts from here. |

## Calls

| Call | Authorized by | State | Effect |
|---|---|---|---|
| `activate()` | Open, decided with inventory in L2/L4 | Preparing | Checks the sale holds `total_tokens`, then moves to Active. |
| `buy(investor, amount)` | `investor` | Active, not paused | Takes `amount` USDC from the investor, measuring the balance received. Reserves `amount / price` tokens and consumes allowlist room. `amount` must be an exact multiple of `price`. |
| `finalize()` | owner | Active | Requires `min_success_percent` sold, or everything sold. Moves to Successful. |
| `mark_failed()` | owner | Active | Requires the minimum not sold. Moves to Failed. |
| `cancel()` | owner | Active | Moves to Failed at any time. Base's `enableRefunds`. |
| `release(investors)` | anyone | Successful | For each investor, at most 30: transfers the reserved tokens and ends the reservation. Investors with nothing reserved are skipped. Fails if any investor's cooling-off window is still open. |
| `refund(investors)` | anyone | Failed | For each investor, at most 20: returns the payment, ends the reservation and restores allowlist room. Investors with nothing reserved are skipped. |
| `cooling_off_refund(investor)` | `investor` | Active or Successful | Within `cooling_off_secs` of the last purchase, and before release: returns the payment, ends the reservation and restores allowlist room. |
| `withdraw_payment(payouts)` | owner | Successful | Pays every `(recipient, amount)` in one call. |
| `withdraw_tokens(to, amount)` | owner | Failed | Returns tokens to the issuer. |
| `pause()` | owner or `pauser` | any | Blocks `buy`. |
| `unpause()` | owner | any | Resumes `buy`. |
| `upgrade(new_wasm_hash, operator)` | `upgrader` | any | Replaces the code. |

Read functions: `state`, `config` (price, totals, hard cap, cooling-off, minimum), `sold_tokens`, `raised`, `reservation(investor)`, `cooling_off_ends_at(investor)`, `preview_tokens(amount)`, `paused`.

Differences from Base:
- **Release waits for the cooling-off window** (audit SCAN-01, recommended fix). In Base, anyone could release early.
- **`withdraw_payment` takes every payout at once.** It replaces several `withdrawBrla` calls batched in one Safe transaction.

Points still open for L4 are listed in [interface 11](interface.md#11-open-points):
- skip-and-report in batches;
- leftover tokens after success;
- reserving funds for open cooling-off windows;
- what pause blocks.

## Events

| Event | Topics | Data | When |
|---|---|---|---|
| `state_changed` | `state_changed` | `{from, to}` | Every transition |
| `activated` | `activated` | `{tokens}` | `activate` |
| `purchased` | `purchased`, investor | `{last_purchase_at, paid, reserved_paid, reserved_tokens, tokens}` | `buy`. `paid` and `tokens` are this purchase; `reserved_*` are the investor's totals afterwards. |
| `finalized` | `finalized` | `{raised, sold_tokens}` | `finalize` |
| `marked_failed` | `marked_failed` | `{raised, sold_tokens}` | `mark_failed` |
| `cancelled` | `cancelled` | `{raised, sold_tokens}` | `cancel` |
| `released` | `released`, investor | `{tokens}` | `release`, per investor served |
| `refunded` | `refunded`, investor | `{paid, reason, tokens}` | `refund` (`reason` = `failed`) and `cooling_off_refund` (`reason` = `cooling_off`) |
| `payment_withdrawn` | `payment_withdrawn`, recipient | `{amount}` | `withdraw_payment`, per payout |
| `tokens_withdrawn` | `tokens_withdrawn`, to | `{amount}` | `withdraw_tokens` |
| `upgraded` | `upgraded` | `{new_wasm_hash, schema_version}` | `upgrade` |

Inherited from OpenZeppelin: `paused`, `unpaused`, and the role and ownership events. The token emits its own `transfer` for every release, and the USDC contract emits a `transfer` for every purchase, refund and payout.

`reason` is a symbol. `from` and `to` in `state_changed` are symbols: `preparing`, `active`, `successful`, `failed`.

## Errors

| Code | Name | When |
|---|---|---|
| 6200 | `NotPreparing` | `activate` outside Preparing |
| 6201 | `NotActive` | `buy`, `finalize`, `mark_failed` or `cancel` outside Active |
| 6202 | `NotSuccessful` | `release` or `withdraw_payment` outside Successful |
| 6203 | `NotFailed` | `refund` or `withdraw_tokens` outside Failed |
| 6204 | `InventoryMissing` | `activate` without the whole supply held |
| 6205 | `ZeroAmount` | `buy` with nothing to pay |
| 6206 | `NotWholeTokens` | `buy` with an amount that isn't an exact multiple of the price |
| 6207 | `ExceedsSupply` | `buy` beyond the tokens left |
| 6208 | `HardCapExceeded` | `buy` beyond the hard cap |
| 6209 | `InsufficientReceived` | Less USDC arrived than the amount paid |
| 6210 | `SuccessCriteriaUnmet` | `finalize` below the minimum |
| 6211 | `SuccessCriteriaMet` | `mark_failed` at or above the minimum |
| 6212 | `CoolingOffClosed` | `cooling_off_refund` outside the window, after release, or in Failed |
| 6213 | `NothingReserved` | `cooling_off_refund` with no reservation |
| 6214 | `CoolingOffOpen` | `release` that includes an investor whose window is still open |
| 6215 | `BatchTooLarge` | More than 30 investors in `release`, 20 in `refund`, or 30 payouts |
| 6216 | `InvalidConfig` | Constructor arguments below the minimums |

Codes from the contracts it calls, which surface unchanged:
- **allowlist:** 6100 `InvestorNotEnabled` and 6101 `AllocationExceeded` during `buy`;
- **token:** 6000 and 100;
- **USDC:** a balance or trustline error during a payment, refund or payout;
- **OpenZeppelin:** 1000 `EnforcedPause` and 2000 `Unauthorized`.

## Storage

- One persistent reservation per investor, extended on every touch.
- State, configuration, totals, pause flag and schema version: instance, extended on every call.

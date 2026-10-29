# BWB on Stellar

Soroban contracts and Stellar integration code for BWB Digital Assets. This is
the public side of the platform; the application, its Convex backend and the
EVM contracts live in a private repository.

**Status:** nothing is deployed to mainnet. No external audit has been
performed yet.

## Layout

| Path | What | Owner |
|---|---|---|
| `contracts/` | Soroban contracts, one crate each (see table below) | contracts |
| `scripts/` | `deploy.sh`: build, upload, deploy, record | contracts |
| `deployments/` | Per-network manifests with contract IDs and WASM hashes | contracts |
| `docs/` | Backend ↔ contracts interface document and one doc per contract | contracts |
| `sdk/` | TypeScript adapters (stubs) | BWB team |
| `audit/` | Placeholder for audit reports | BWB team |

`indexer/` is reserved for the contract-event indexer in a later tranche.

### Contracts

The contracts port BWB's Base (EVM) contracts to Soroban with the same
semantics, built on [OpenZeppelin Stellar Contracts](https://github.com/OpenZeppelin/stellar-contracts) 0.7.2.

| Crate | Base counterpart | Role | Tranche |
|---|---|---|---|
| `offer-token` | `OfferERC20` | Investor position in one offering; transfer whitelist, pause, admin transfer, metadata ([docs](docs/offer-token.md)) | 1 |
| `offer-allowlist` | `OfferSaleAllowlist` | Who may invest and up to how much, per UTC month; one global instance ([docs](docs/offer-allowlist.md)) | 1 |
| `offer-sale` | `OfferTokenSale` | Escrow and lifecycle: reserve, close, finalize, cancel, cooling-off, refund, release ([docs](docs/offer-sale.md)) | 1 |

`offer-token-upgrade-fixture`, `offer-allowlist-upgrade-fixture` and `offer-sale-upgrade-fixture` (each under its crate's
`upgrade-fixture/`) are test-only second versions of the contracts, used by their upgrade tests. They are never deployed.

Distributors and factories follow in Tranche 2.

### Testnet

From [`deployments/testnet.json`](deployments/testnet.json). The allowlist is global; each offering has a token and a sale contract. `t1-demo-4` and `t1-demo-5` are the demonstration's offerings; `l2-demo-3` is L2's, whose sale address was never deployed. The allowlist and the four contracts of `t1-demo-4` and `t1-demo-5` were deployed fresh on 2026-10-27 from commit `e888a22`; on 2026-10-28 both sale contracts were upgraded in place to the WASM of the branch head, so `stellar contract build` of the head gives every hash below. The two sales still report `schema_version` 2: the constructor stored it under the earlier build, and the head's constant is 1; nothing else differs.

| Contract | Contract ID | WASM hash | Deploy |
|---|---|---|---|
| `offer-allowlist` 0.6.0 | [`CAVI4RROECISQXR4BR3O2BYH6NJFE7WI6NU6UTTHZKX3VCR6J6PWEDRQ`](https://stellar.expert/explorer/testnet/contract/CAVI4RROECISQXR4BR3O2BYH6NJFE7WI6NU6UTTHZKX3VCR6J6PWEDRQ) | `577b1c184ec72a065d1a7e5dcc462de0343ac63af49de8c758fdb0d77f7e5597` | [`826dd754…`](https://stellar.expert/explorer/testnet/tx/826dd75483610ff9a91c4fb488447b1c69fe0cd5cbcd8ecccf885d8d3603b517) |
| `offer-token` t1-demo-4 (0.2.0) | [`CACNDXN74YKAIIGATVUZCSZHAKKWA5MARPX7FV4IEIDPGYIN6XLSZZWT`](https://stellar.expert/explorer/testnet/contract/CACNDXN74YKAIIGATVUZCSZHAKKWA5MARPX7FV4IEIDPGYIN6XLSZZWT) | `a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0` | [`a9f4dd7c…`](https://stellar.expert/explorer/testnet/tx/a9f4dd7cc2882ccb4ac0d82e50232cc5a653cc534eae7803fcd6513ad1eece2a) |
| `offer-sale` t1-demo-4 (0.2.0) | [`CAYQ6SOR3AL4XOTFMBEUATJ56X2KLSTUUJVXKXXYYHJWCKJIEJNDQJUT`](https://stellar.expert/explorer/testnet/contract/CAYQ6SOR3AL4XOTFMBEUATJ56X2KLSTUUJVXKXXYYHJWCKJIEJNDQJUT) | `49edfc275630882ce01e9a190a84f0f833ad3e48183ddd17ea2487463b0af6bd` (upgraded in place from `7a6d8026…`, [tx](https://stellar.expert/explorer/testnet/tx/779a16b639b7dfbfb7c360ba8477bdcd3d4b1e512f6964d615e8920ef50fede1)) | [`a9855097…`](https://stellar.expert/explorer/testnet/tx/a985509755d5127c6256d38e38c2ae83e2d1e750ad9c0f97989efe2d56522631) |
| `offer-token` t1-demo-5 (0.2.0) | [`CDSY2PJZUBY5EQCYEP2QGZE7GP3NF3IUFRQ2A7CTJLZQHN2Y7F343653`](https://stellar.expert/explorer/testnet/contract/CDSY2PJZUBY5EQCYEP2QGZE7GP3NF3IUFRQ2A7CTJLZQHN2Y7F343653) | `a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0` | [`80fccdd7…`](https://stellar.expert/explorer/testnet/tx/80fccdd77e86435110c654ce909b0613f37151f424c5bbe894472b0c160e2a32) |
| `offer-sale` t1-demo-5 (0.2.0) | [`CAGFA4RWRBBATYCJQAE5O5GECPTOKMLDVHM4R5BR454JN6UOCIKYDFER`](https://stellar.expert/explorer/testnet/contract/CAGFA4RWRBBATYCJQAE5O5GECPTOKMLDVHM4R5BR454JN6UOCIKYDFER) | `49edfc275630882ce01e9a190a84f0f833ad3e48183ddd17ea2487463b0af6bd` (upgraded in place from `7a6d8026…`, [tx](https://stellar.expert/explorer/testnet/tx/65e74d917e0c2f7033e3289a7d19ee9aad79391e8cd461f8cb69b70f21cd9ade)) | [`4ea3f9a8…`](https://stellar.expert/explorer/testnet/tx/4ea3f9a8e90919308d15d6b63eea91eec0168a7d0abc5081e3244bdda659b1a1) |
| demo USDC (payment asset) | [`CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY`](https://stellar.expert/explorer/testnet/contract/CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY) | a Stellar Asset Contract for `USDC:GDGBRCFA…`, issued by the `bwb-testnet-usdc` identity; not Circle's | — |
| `offer-token` l2-demo-3 (0.1.0) | [`CBX74ZZUBLYPBNMND7DWTU74I4QWUIQEJZNH2XJVBX3THZ2KDRRFMESM`](https://stellar.expert/explorer/testnet/contract/CBX74ZZUBLYPBNMND7DWTU74I4QWUIQEJZNH2XJVBX3THZ2KDRRFMESM) | `40bf9cc67d2cf9e0d2b3c238f8e5b0f3ed3dfb016998453d4ddfc0069037f385` | [`4a9856e2…`](https://stellar.expert/explorer/testnet/tx/4a9856e2587fe1cfade5a83f359a85260d70abbecb95507e5c8cae79c4888a6f) |

Each token's whole supply is minted to its sale contract's precomputed address. The allowlist's admin is the `bwb-testnet-platform` identity and its only allocator is the deployer hot key; each sale contract is granted `sale` at its own deploy, signed by the allocator. The sale contracts' owner is the `bwb-testnet-treasury` identity, their pauser and upgrader the platform. Earlier allowlist instances are superseded and unused: `CASHJ3GS…WQKZYT6A` (0.6.0, the first `t1` run's; investor A had used its October room there), `CAWQW2XW…IIX3THF` (0.5.0, the first L4 run's, upgraded in place from `c60cbd64…`), `CBQLP3DC…TQQHJ26` (0.3.0, the receipt book still named `Receipts`), `CDMGBEK5…TJTH7JSD` (0.2.0, where `restore` still required the `sale` role), `CBQCDFMM…656AT6LX` (receipts numbered per sale contract, which made concurrent purchases conflict), `CAWUBAUY…ESWRBKF` (restore by period and amount, before per-purchase receipts) and `CBTU5S5V…KC42XU` (the single-role code before the audit).

Superseded offerings, kept in the manifest: `t1-demo-1` and `t1-demo-3`, the first `t1` run on 2026-10-26, with sale contracts from `a0a9dd8` (`offer-sale` 0.1.0, the accounting still stored as `Totals`, so they cannot be upgraded to 0.2.0); `t1-demo-1` stays in Closing, its stage 2 never run. `l4-demo-1` and `l4-demo-2`, the first L4 run on 2026-10-22, with the 0.5.0 allowlist and sale contracts from before the audit rounds (`7367f872…`); `l4-demo-1` stays in Closing, its stage 2 never run. The offering id `t1-demo-2` is taken on testnet by an L2-era token (`CAWMEO3F…O4TSENQT`, older code), so the failure path runs on `t1-demo-3`; the sale contract deployed at `t1-demo-2`'s address (`CA23WY2X…FP2AWSAL`) points at that token, was never activated and is unused. Neither is in the manifest.

### L4 demonstration

`scripts/demo-offer-sale.sh` runs the grant's demonstration on the two offerings above, each step a transaction with its explorer link; rejected calls are submitted anyway, so each rejection is a failed transaction on the ledger with its error code. Stage 1 ran on 2026-10-27 (steps 1–15 on `t1-demo-4`, the success path up to the close; 16–22 on `t1-demo-5`, the failure path); the demo USDC issuer first topped the investors up ([`28fc5e8b…`](https://stellar.expert/explorer/testnet/tx/28fc5e8b99dd9c64415b907b9771d13939acd47b3830a79cd1f8f0a605afe604), [`ec444bf5…`](https://stellar.expert/explorer/testnet/tx/ec444bf5238b470a8fe6b4d2ca8f292e96c1a3bc1de72d7420646ec1b4bee74f)). Stage 1 ran on the sale contracts' first WASM, `7a6d8026…`; on 2026-10-28, before stage 2, both sale contracts were upgraded in place to the branch head's `49edfc27…` with their storage kept ([`779a16b6…`](https://stellar.expert/explorer/testnet/tx/779a16b639b7dfbfb7c360ba8477bdcd3d4b1e512f6964d615e8920ef50fede1) for `t1-demo-4`, [`65e74d91…`](https://stellar.expert/explorer/testnet/tx/65e74d917e0c2f7033e3289a7d19ee9aad79391e8cd461f8cb69b70f21cd9ade) for `t1-demo-5`). Stage 2 (`STAGE=2`, steps 23–26 on `t1-demo-4`: finalize with the burn of the 200 unsold tokens, release, a refused late cooling-off and the payout) ran on 2026-10-29, after `t1-demo-4`'s last cooling-off window ran out on 2026-10-28 21:05:06 UTC (`finalizable_at` 1793221506, also on the `closed` event). The offering ended Successful: 800 of 1,000 tokens sold for 8,000 USDC, 200 burned, 800 released to the two investors, and the 8,000 USDC paid out in two legs, 7,000 to the treasury and 1,000 to investor B. One stray transaction sits between steps 24 and 25: [`f80ed1ca…`](https://stellar.expert/explorer/testnet/tx/f80ed1ca92e5058a01881866f58b743a382f28694ac7097194706f1b19d162c1) is a first attempt at step 25 that failed with a footprint trap (`trying to access contract code outside of the footprint`) because the manifest on `main` still named the pre-upgrade WASM `7a6d8026…`, which the helper put in the footprint; it is not the 6225 evidence. The step was rerun once the manifest carried `49edfc27…`.

| Step | What | Transaction |
|---|---|---|
| 1 | The allocator grants the sale contract the allowlist's sale role | [`8b54be8d…`](https://stellar.expert/explorer/testnet/tx/8b54be8d10afd9a815ef0dcf979c6c2c9e05d6efe1a0810c4e5805084fe92fa8) |
| 2 | Anyone activates: the sale holds the whole supply and the role | [`9bc461f8…`](https://stellar.expert/explorer/testnet/tx/9bc461f8d2b043eaa4d942e6f54f2130a966d7957999b88769be12f80af9144c) |
| 3 | The allocator allows investor A up to 3,000 USDC this month | [`9e9c86c0…`](https://stellar.expert/explorer/testnet/tx/9e9c86c006aaccaa0ed55031b6a489cc9fa74978b5d8c6d705e5c1e15e074bb8) |
| 4 | Eligibility rejection: investor B, not on the allowlist, buys 10 tokens (6100 InvestorNotEnabled) | [`bb98c609…`](https://stellar.expert/explorer/testnet/tx/bb98c60984c32ada9b0fb770579bbffbf31102d51305df9d3e2ef468f44b96a6) |
| 5 | Allocation rejection: investor A buys 400 tokens, 4,000 USDC over a 3,000 cap (6101 AllocationExceeded) | [`da1ac6bf…`](https://stellar.expert/explorer/testnet/tx/da1ac6bf4aa09f9e2d1b657878ae119771c1892c1c8143e884d5420dafd3ec97) |
| 6 | Purchase: investor A buys 50 tokens for 500 USDC | [`490380f8…`](https://stellar.expert/explorer/testnet/tx/490380f877d4d7250b0819cded99fd51c2ab1aab8a578fa0f74f11f4c41b2f62) |
| 7 | The platform pauses the sale | [`47d1debb…`](https://stellar.expert/explorer/testnet/tx/47d1debb3855e60e43915ef0bf49929d41a3d98c3e69a7c76ef40ae18bd56513) |
| 8 | Purchase while paused: rejected (1000 EnforcedPause) | [`e5ca35a6…`](https://stellar.expert/explorer/testnet/tx/e5ca35a6225c7795f1ef2360046548730293f7b689c55662b0d12cf2225f9d3a) |
| 9 | The platform tries to unpause: rejected, only the owner unpauses (6222 NotOwner) | [`58ff8d0a…`](https://stellar.expert/explorer/testnet/tx/58ff8d0a4b254bb963d2d73f66aa195cd6dad29bc5399fe0212ec2525c05416f) |
| 10 | The owner unpauses | [`813e200b…`](https://stellar.expert/explorer/testnet/tx/813e200b690a169db6c3dee43b3deba7edb32db810305ed1134b6708b8c2c5e2) |
| 11 | Cooling-off: investor A undoes the purchase and gets the 500 USDC back; the allowlist room returns | [`8833a61e…`](https://stellar.expert/explorer/testnet/tx/8833a61e85e46b46cf1d6eecd96735184b0b24427e863c40605b2e94f03f37e7) |
| 12 | Purchases: investor A buys 300 tokens; the allocator allows investor B, who buys 500 | [`4ad53f5c…`](https://stellar.expert/explorer/testnet/tx/4ad53f5c3438a13a08ff0bb7ffa22227f11295b40995e983e0a847ea993a70ad), [`8b8e2cf0…`](https://stellar.expert/explorer/testnet/tx/8b8e2cf0dd679372f48c5d2a554bd1bcba65e6d258d5d9d7983c7100ae4d48de), [`51cb1e7a…`](https://stellar.expert/explorer/testnet/tx/51cb1e7a2cddfbc30ae9de361d945a996ad8cc8be000ff24fe7fdc699a5fbb2a) |
| 13 | The owner closes buying, reopens, and closes again | [`ab9e107a…`](https://stellar.expert/explorer/testnet/tx/ab9e107adf1ef46c427ee31bfc3c6980066af660930149d96be31f5959eab472), [`8bd3c43a…`](https://stellar.expert/explorer/testnet/tx/8bd3c43a7a6c5484e63127e72b82f851947e7a48d752feeb83524fab3aee6d18), [`199a5121…`](https://stellar.expert/explorer/testnet/tx/199a5121db9c77df0c92fc5d2e12085e81ccd464695dbd02fe38647ee33d00a8) |
| 14 | Purchase after close: rejected (6201 NotActive) | [`6a5aa429…`](https://stellar.expert/explorer/testnet/tx/6a5aa42956df549438875a0f11812eeef94ff0d30e10c6d7031cb2f9c7661909) |
| 15 | Finalize before the windows run out: rejected (6214 CoolingOffOpen) | [`d10854ca…`](https://stellar.expert/explorer/testnet/tx/d10854caec35581776c0134a380d65522b8a9dd975bf7fe8ff6c11b09d4ad8e3) |
| 16 | The allocator grants the sale contract the allowlist's sale role | [`835bd395…`](https://stellar.expert/explorer/testnet/tx/835bd3956b5254ae820aa0cbaad4b1e8ac41f60a5f236fb5a6cad1d639ad4a53) |
| 17 | Anyone activates: the sale holds the whole supply and the role | [`32b2c3b2…`](https://stellar.expert/explorer/testnet/tx/32b2c3b2923ad90ee00bcae8daa9f3333cef9a70544ac81737c1a2bf9c61a299) |
| 18 | The allocator raises investor A's cap to 4,000 USDC (the cap is per month across offerings); A buys 100 tokens | [`f4fd5d69…`](https://stellar.expert/explorer/testnet/tx/f4fd5d69519b09839cae2c414e5f98d0d2fa628d160f83c108d424a1b9b97403), [`9b3a9474…`](https://stellar.expert/explorer/testnet/tx/9b3a9474451c883045be4005320064208dc1d8c64c9ded5b59da33bb83d5e5a5) |
| 19 | Investor B tries to cancel the offering: rejected (6222 NotOwner) | [`b6c2da8d…`](https://stellar.expert/explorer/testnet/tx/b6c2da8dbd381c7884b1d3bd8e1e5643920fbc94ef0b2571dcaa55909c44b0e1) |
| 20 | The owner cancels the offering | [`7ab128c2…`](https://stellar.expert/explorer/testnet/tx/7ab128c2b8b7ee3a13a8f838a5eb70d7c7ccf87578e68ecc7e614acf85c1fa6a) |
| 21 | Refund: anyone refunds investor A; the money and the allowlist room come back | [`42a52035…`](https://stellar.expert/explorer/testnet/tx/42a52035d494e23b636bb9f4b2b25fedefd8465b1827d340df880fddf6eadef7) |
| 22 | The owner withdraws the tokens of the failed offering | [`2d6740b7…`](https://stellar.expert/explorer/testnet/tx/2d6740b753e66e8ffb072c2f0c3c31ec34904af62d52c892339bfdee900fd6cc) |
| 23 | Anyone finalizes: 800 of 1,000 sold, the 200 unsold tokens are burned; the offering moves to Successful | [`72c1fc24…`](https://stellar.expert/explorer/testnet/tx/72c1fc24aa46ea35ecfb4f6d12fbd4dec2ac9a0644eac8fb8826ba37cbd0ee88) |
| 24 | Release: anyone delivers the tokens to investors A (300) and B (500) | [`40808bac…`](https://stellar.expert/explorer/testnet/tx/40808bac73c623af3d4963c805843f9d6e9321df1fa55017fc55f9eb9580a4c6) |
| 25 | Late cooling-off by investor A: rejected, the offering is decided (6225 NotOpen) | [`68350463…`](https://stellar.expert/explorer/testnet/tx/6835046308df8845fa65d73e37cd224ef2301ad6ad526b985a2c7876f1be3055) |
| 26 | The owner withdraws the 8,000 USDC raised, in two legs: 7,000 to the treasury, 1,000 to investor B | [`04659f8b…`](https://stellar.expert/explorer/testnet/tx/04659f8b71f9386b4f87b3ca3332e443eed23153c00f6e54e9316640c1d9464d) |

## Build and test

Rust toolchain, target and components are pinned in `rust-toolchain.toml`;
`rustup` installs them on first use. The [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli)
(28.x) is needed to build the WASM and to deploy: OpenZeppelin 0.7.2 enables
soroban-sdk's spec-shaking feature, which only builds through the CLI.

```bash
rustup show                     # installs the pinned toolchain and wasm32v1-none
cargo fmt --all -- --check
scripts/test.sh                 # builds the upgrade fixtures, then cargo test --workspace
cargo clippy --all-targets --workspace -- -D warnings
stellar contract build          # WASM to target/wasm32v1-none/release/
```

CI runs exactly these four commands on every pull request. Run the tests through
`scripts/test.sh`: the upgrade tests of the three contracts swap to second
WASMs that only the Stellar CLI builds. A plain `cargo test` before that fails
one test per crate with a hint saying so.

## Deploy

```bash
stellar keys generate <identity> --network testnet --fund
NETWORK=testnet scripts/offering-addresses.sh <offering-id>   # token and sale addresses
NETWORK=testnet SOURCE=<identity> OFFERING=<offering-id> scripts/deploy.sh offer-token -- \
  --power_holders '{"admin":"G...","xfer_admin":"G...","pauser":"G...","upgrader":"G..."}' \
  --metadata '{"name":"...","symbol":"...","offer_uri":"ipfs://..."}' \
  --supply 1000 --initial_holder <sale address> --whitelist '[]'
NETWORK=testnet SOURCE=<identity> OFFERING=<offering-id> scripts/deploy.sh offer-sale -- \
  --power_holders '{"owner":"G...","pauser":"G...","upgrader":"G..."}' \
  --contracts '{"token":"<token address>","payment_asset":"<USDC contract>","allowlist":"<allowlist>"}' \
  --config '{"price":"100000000","cooling_off_secs":432000,"min_success_percent":75}'
```

The sale contract is then granted `sale` on the allowlist by an allocator and
activated by anyone (both in `scripts/demo-offer-sale.sh`).

The global allowlist takes no `OFFERING`; its admin and initial allocators are
constructor arguments:

```bash
NETWORK=testnet SOURCE=<identity> scripts/deploy.sh offer-allowlist -- \
  --admin "$(stellar keys address bwb-testnet-platform)" \
  --allocators "[\"$(stellar keys address bwb-testnet-deployer)\"]"
```

An offering's token and sale contract live at addresses derived from the
deployer and the offering identifier, so both are known before either is
deployed and a retry finds the same ones. The script builds the crate, uploads
the WASM, deploys with the constructor arguments given after `--`, and records
contract ID, salt, WASM hash, deploy transaction, deployer, toolchain versions
and explorer links in `deployments/<network>.json`. Administrative powers are
constructor arguments; the deployer identity only pays fees. Pass `--network`
explicitly to any manual `stellar` command: a `STELLAR_NETWORK` in your
environment otherwise wins.

`OFFERING=<offering-id> scripts/demo-offer-token.sh` then exercises a deployed
token on testnet: transfers allowed and rejected by the whitelist, pause,
admin transfer, unauthorized calls and burn. `STAGE=1 scripts/demo-offer-sale.sh`
and, once the cooling-off window has run out, `STAGE=2` exercise the two demo
sale contracts end to end; both stages ran on testnet (2026-10-27 and
2026-10-29), their hashes are under L4 demonstration above. Rejected calls
are submitted anyway, so each rejection is
a failed transaction on the ledger with its error code
(`scripts/lib/send_rejected.py`): a call that touches only its own storage
names its keys; a call that reaches other contracts, such as a purchase,
reuses the simulated footprint of a passing twin.

## License

Everything in this repository is licensed under [Apache 2.0](LICENSE).

## Contact

- Website: [bwbi.com.br](https://bwbi.com.br)
- Email: contato@bwbi.com.br

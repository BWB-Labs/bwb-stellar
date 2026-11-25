# Contract event indexer

A [Goldsky Turbo](https://docs.goldsky.com/turbo-pipelines) pipeline reads
Stellar and posts the events of the tracked contracts, in batches, to a
receiver. The receiver's own setup is documented with the receiver.

| File | What |
|---|---|
| `render-pipeline.py` | Renders the pipeline YAML and the seed SQL for one network, and fills in the receiver's address for applying |
| `pipelines/<network>.yaml` | The rendered pipeline, committed. The receiver's address is a placeholder |
| `pipelines/<network>.seed.sql` | Creates the tracked-contract tables and adds every contract in [`deployments/<network>.json`](../deployments/testnet.json). Safe to run again |

## What the pipeline does

One pipeline per network, with two streams, one worker and one checkpoint.

| Stream | Source | Path | Columns |
|---|---|---|---|
| Contract events of the tracked contracts, up to 100 rows per request | `stellar_<network>.events` | `/webhooks/goldsky/events` | `id`, `contract_id`, `topics`, `data`, `transaction_hash`, `transaction_successful`, `in_successful_contract_call`, `ledger_sequence`, `ledger_closed_at`, `chain_id`, `contract_kind` |
| Heartbeat, one ledger every 720 (about one an hour) | `stellar_<network>.ledgers` | `/webhooks/goldsky/heartbeat` | `ledger_sequence`, `ledger_closed_at`, `chain_id` |

The contracts can be quiet for days: without the heartbeat a stalled pipeline
and an idle chain look the same. The streams read the chain at their own pace,
so a heartbeat at the chain head does not mean the events have caught up.

`chain_id` is the SHA-256 of the network passphrase, which is Stellar's network
id. `contract_kind` is `token`, `sale`, `allowlist` or `asset`, taken from the
table that tracks the contract. `id` is the Stellar event id.

## Tracked contracts

Four Postgres tables, one per kind: `streamling.<network>_tracked_tokens`,
`_sales`, `_allowlists` and `_assets`, in a Goldsky-hosted database (secret
`BWB_INDEXER_POSTGRES`). The pipeline reads them as
[dynamic tables](https://docs.goldsky.com/turbo-pipelines/transforms/dynamic-tables)
on every batch, so inserting a contract id tracks it within seconds and deleting
the row stops it, with no apply and no restart. Ids are stored as Stellar writes
them, in upper case; the lookup is an exact match. The seed creates the tables,
adds the manifest's contracts and grants `INSERT` to the role `bwb_backend`,
which must exist before it runs.

A contract added later only gets events from then on. To recover earlier ones,
restart the pipeline from an earlier ledger (see [Operations](#operations)). A
receiver should ignore events whose `id` it already holds.

## Render and apply

```bash
python3 indexer/render-pipeline.py --network testnet --start-at 5177900
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f indexer/pipelines/testnet.seed.sql
```

This writes `pipelines/testnet.yaml` and `.seed.sql`; commit both. Run the seed,
with the hosted database's address in `DATABASE_URL`, before the first apply.

| Flag | Default | Meaning |
|---|---|---|
| `--network` | `testnet` | Network; `deployments/<network>.json` seeds it |
| `--start-at` | required | `latest`, `earliest` or a ledger sequence. Also where a replay starts, so re-render with the committed value unless it is meant to move |
| `--heartbeat-every` | `720` | Ledgers between heartbeats |
| `--resource-size` | `xs` | Goldsky worker size |
| `--name` | `bwb-stellar-<network>` | Pipeline name |
| `--secret-name` | `BWB_GOLDSKY_WEBHOOK` | Goldsky secret holding the `Authorization` header |
| `--out` | `pipelines/<network>.yaml` | Output file |

The sink urls start with the placeholder `${WEBHOOK_MANAGER_URL}`. The
receiver's address contains a secret, a Goldsky sink cannot read its url from a
secret, and this repository is public, so the filled copy is never committed:

```bash
read -rs WEBHOOK_MANAGER_URL && export WEBHOOK_MANAGER_URL
python3 indexer/render-pipeline.py --network testnet --fill
goldsky turbo validate indexer/pipelines/testnet.local.yaml
goldsky turbo apply indexer/pipelines/testnet.local.yaml
rm indexer/pipelines/testnet.local.yaml && unset WEBHOOK_MANAGER_URL
```

`--fill` only copies the committed file: it refuses render flags, checks the
shape of the address, and writes only to a path ending in `.local.yaml`.
`apply` is an upsert that keeps the checkpoint, which Goldsky keys on the
pipeline's name and its source names: a rename replays from `start_at`.

The `Authorization` header comes from a Goldsky `httpauth` secret. The receiver
needs the same token; remove `/tmp/goldsky-token` once it has it.

```bash
(umask 077 && openssl rand -hex 32 > /tmp/goldsky-token)
goldsky secret create --name BWB_GOLDSKY_WEBHOOK \
  --value "{\"type\":\"httpauth\",\"secretKey\":\"Authorization\",\"secretValue\":\"Bearer $(cat /tmp/goldsky-token)\"}"
```

## What a receiver must offer

Two HTTPS endpoints under one base address, `/webhooks/goldsky/events` and
`/webhooks/goldsky/heartbeat`, that accept a batch of rows and check the bearer
token in the `Authorization` header. The checkpoint moves when the receiver
answers 200; Goldsky fails the pipeline on any 4xx other than 408 and 429,
which stops both streams, and retries otherwise.

| Answer | Meaning | Pipeline |
|---|---|---|
| 200 | Batch stored, or already held | Checkpoint moves |
| 400 | Body is not rows | Fails |
| 401 | Missing or wrong token | Fails |
| 422 | Rows of another `chain_id` | Fails |
| 500 | The write failed | Retries |

## Operations

```bash
goldsky turbo list                                         # running, paused, error
goldsky turbo state list bwb-stellar-testnet               # each source's checkpoint
goldsky turbo logs bwb-stellar-testnet
goldsky turbo pause bwb-stellar-testnet
goldsky turbo resume bwb-stellar-testnet
goldsky turbo restart bwb-stellar-testnet --clear-state    # replay from start_at
```

To replay from another ledger, re-render with that `--start-at`, fill, apply,
then `restart --clear-state`. A paused pipeline leaves the hosted database idle.

**Testnet reset.** Stellar resets testnet periodically and every contract id is
gone. Pause the pipeline, redeploy with [`scripts/deploy.sh`](../scripts/deploy.sh),
then empty the tables:

```bash
psql "$DATABASE_URL" -c 'TRUNCATE streamling.testnet_tracked_tokens, streamling.testnet_tracked_sales, streamling.testnet_tracked_allowlists, streamling.testnet_tracked_assets'
```

Then re-render with `--start-at` just before the first new deploy, commit, run
the seed, fill, apply and `restart --clear-state`.

**Mainnet.** The same steps with `--network mainnet`, once `deployments/mainnet.json`
exists, with its own token and secret. Its tables are `streamling.mainnet_tracked_*`
in the same database. Tracking USDC would deliver every USDC event on the network.

# Contract event indexer

Every event the BWB contracts emit on Stellar is delivered to the backend by a
[Goldsky Turbo](https://docs.goldsky.com/turbo-pipelines) pipeline: Goldsky
reads the chain, keeps the events of the tracked contracts and posts them in
batches to the backend's Webhook Manager. The tracked contracts are rows in a
Postgres database the pipeline consults as it runs, so a contract is tracked
the moment its row exists: the backend adds the contracts it deploys before it
deploys them, with no script and no Goldsky command. The manager queues
each batch and hands it to every backend deployment listening to it; each one
writes the batch before it acknowledges it and keeps one row per Stellar event
id. A batch a backend did not take is retried by the manager and then kept
there as a failed delivery. No poller, no cursor of ours.

This directory holds the pipeline definition and the script that renders it.
The receiver (routes, a Convex component with the tables and the decoder,
liveness cron) and the Webhook Manager live in the private backend repository,
with the reasoning in that repository's ADR 0022 and ADR 0023.

| File | What |
|---|---|
| `render-pipeline.py` | Renders a pipeline YAML for one network and the SQL that seeds its tracked contracts, and fills in the manager's address for applying |
| `pipelines/<network>.yaml` | The rendered pipeline, committed so a diff shows what changed. The manager's address is a placeholder |
| `pipelines/<network>.seed.sql` | Creates the tracked-contract tables and adds every contract `deployments/<network>.json` references. Safe to run again |
| `pipelines/<network>.local.yaml` | The filled copy that is applied. It holds a secret and is ignored by git |

## How it works

One pipeline per network, two streams in it, one worker, and one checkpoint
that moves only when both sinks have acknowledged. The two streams read the
chain at their own pace, so the heartbeat reaching the chain head does not by
itself mean the events have:

- **Contract events.** `stellar_<network>.events` filtered in SQL to the
  tracked contracts. Each row carries the network as `chain_environment` and
  the contract's kind (`token`, `sale`, `allowlist`, `asset`) as
  `contract_kind`, so the backend never guesses the kind from an event name two
  contracts share. Delivered to `/webhooks/goldsky/events`.
- **Heartbeat.** `stellar_<network>.ledgers` filtered to one ledger every 720
  (about one an hour at 5 s per ledger). The contracts are quiet for days, so
  without it a stalled pipeline and an idle chain look the same. Delivered to
  `/webhooks/goldsky/heartbeat`; the backend marks the feed stale when neither a
  heartbeat nor an event has arrived for two hours.

**Tracked contracts.** Four tables in a Postgres database hosted by Goldsky,
one per contract kind: `streamling.<network>_tracked_tokens`, `_sales`,
`_allowlists` and `_assets`. Each is one column of contract ids. The pipeline
declares them as [dynamic tables](https://docs.goldsky.com/turbo-pipelines/transforms/dynamic-tables)
and asks them, on every batch, whether an event's contract is tracked and as
which kind. A row added while the pipeline runs takes effect within seconds,
with no apply and no restart. Two writers add rows:

- the backend, for every offering it deploys: before it submits the deploy
  transaction it adds the token, the sale and the allowlist, and it does not
  deploy until they are in. A contract's address is known before it exists, so
  its first events, the ones its constructor emits, are already tracked. It
  does not add the payment asset, which for its offerings is Circle's USDC:
  tracking that would deliver every USDC event on the network;
- `pipelines/<network>.seed.sql`, for the contracts `scripts/deploy.sh` wrote to
  the manifest.

Contract ids are stored exactly as Stellar writes them, in upper case: the
lookup is an exact match, and a lower-case row matches nothing.

Both sinks send the `Authorization` header from a Goldsky secret, and the
manager passes it on with the body, unchanged.

**What Goldsky sees.** Only the manager. It answers 200 as soon as a batch is
queued, and the pipeline's checkpoint moves then, before any backend holds the
rows. The manager answers 401 to a wrong hook secret in the address and 404 to
a path it does not know; Goldsky fails the pipeline on any 4xx except 408 and
429, and that stops both streams. A manager that is down is retried forever.

**What the manager sees.** Each backend deployment answers:

| Status | Meaning | Manager |
|---|---|---|
| 200 | batch written (or already held) | done |
| 401 | bad or missing token, or the backend has no token configured | retries, then keeps the batch |
| 400 | body is not rows | retries, then keeps the batch |
| 422 | wrong chain environment | retries, then keeps the batch |
| 500 | the write failed | retries, then keeps the batch |

The manager retries a backend 11 times over about 17 minutes. After that the
batch is logged and kept seven days as a failed delivery, to be replayed by hand
(see [When a batch did not land](#when-a-batch-did-not-land)). Batches can reach
a backend out of order and more than once; it keeps one row per event id and
only ever moves its heartbeat forward.

A row the decoder cannot read is still stored, with its raw topics and data and
the reason, and counted as `undecodable` on the delivery; once the decoder is
fixed, `bunx convex run contractEvents:redecode` on the deployment re-reads
every row below the current decoder version. The pipeline never has to replay
for a decoder defect.

## Setup

Done once per network. Accounts, cards and team membership are a person's to
create; the rest is CLI.

1. **Goldsky.** Install the CLI (`curl https://goldsky.com | sh`) and its Turbo
   extension (`curl https://install-turbo.goldsky.com | sh`), then `goldsky login`
   and `goldsky project switch <project id>` to the team's project
   (`goldsky project list` shows the active one). The project must be on the
   Scale plan: the hosted database below is not offered on Starter. Scale
   includes 750 pipeline worker-hours a month and `xs` uses half a worker; the
   database is billed by the CPU-hour while a pipeline uses it. The team's
   usage page has a spend limit that pauses everything when reached, so set it
   above the database's monthly cost.
2. **Tracked-contract database.** Once per Goldsky project; every network's
   tables live in it:
   ```bash
   goldsky hosted-sink create --type postgres --name BWB_INDEXER_POSTGRES
   ```
   That provisions the database and a Goldsky secret of the same name, which
   the pipeline reads. Its connection details are in the dashboard under Sinks
   (or `goldsky secret reveal BWB_INDEXER_POSTGRES`); they are a secret and go
   to the team's password manager. They are the database owner's, for the
   pipeline and for the steps here. The backend gets a user of its own that can
   only add rows; create it once, with a password of its own:
   ```bash
   read -rs DATABASE_URL && export DATABASE_URL   # paste postgresql://<owner>:<password>@<host>/<database>?sslmode=require
   (umask 077 && openssl rand -hex 24 > /tmp/backend-db-password)
   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
   \set pw `cat /tmp/backend-db-password`
   CREATE ROLE bwb_backend LOGIN PASSWORD :'pw';
   SQL
   ```
   Then create the tables and add the manifest's contracts:
   ```bash
   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f indexer/pipelines/testnet.seed.sql
   unset DATABASE_URL
   ```
   Run it before the first apply, so the contracts are tracked from the first
   ledger the pipeline reads. The backend's address is the owner's with
   `bwb_backend` and its password in place of the owner's; it goes to the
   password manager too, and `/tmp/backend-db-password` is removed.
3. **Token.** Generate one and keep it out of the shell history:
   ```bash
   openssl rand -hex 32 > /tmp/goldsky-token && chmod 600 /tmp/goldsky-token
   ```
4. **Goldsky secret**, type `httpauth`, which Goldsky adds as a header to every
   request of a sink that names it:
   ```bash
   goldsky secret create --name BWB_GOLDSKY_WEBHOOK --description "Authorization header for the BWB Convex webhook routes" \
     --value "{\"type\":\"httpauth\",\"secretKey\":\"Authorization\",\"secretValue\":\"Bearer $(cat /tmp/goldsky-token)\"}"
   ```
5. **Backend settings** on every Convex deployment that listens to the manager
   (from the backend repository's `app/`). All listeners of one manager hold the
   same token, and the same database address, which is how a deployment adds
   the contracts it deploys:
   ```bash
   bunx convex env set GOLDSKY_WEBHOOK_TOKEN "$(cat /tmp/goldsky-token)"
   bunx convex env set CHAIN_ENVIRONMENT testnet
   read -rs BACKEND_DATABASE_URL && bunx convex env set CONTRACT_TRACKING_DATABASE_URL "$BACKEND_DATABASE_URL"; unset BACKEND_DATABASE_URL   # postgresql://bwb_backend:<its password>@<host>/<database>?sslmode=require
   ```
   A deployment without the database address refuses to deploy a Stellar
   offering rather than deploy one nobody indexes.
   `CHAIN_ENVIRONMENT` is the deployment's one chain environment (that
   repository's ADR 0011): `mainnet` on production, `testnet` everywhere else.
   Confirm the routes answer: `curl https://<deployment>.convex.site/webhooks/goldsky/health`
   returns 503 with `"live": false` until the first delivery, which is correct.
   A listener without the token answers 401 to every batch and fills the
   manager's failed deliveries.
6. `rm /tmp/goldsky-token`, and keep the token in the team's password manager:
   every new listener needs it.
7. **Webhook Manager.** The manager of the network's chain environment must
   know the two Goldsky paths (it does once the backend change that adds them is
   pushed to it) and have each backend deployment registered as a listener. Both
   are covered in the backend repository's `webhooks-manager/README.md`;
   previews register themselves.

## Deploy

```bash
indexer/render-pipeline.py --network testnet --start-at 5177900
```

That writes `indexer/pipelines/testnet.yaml` and `indexer/pipelines/testnet.seed.sql`.
Commit both. The pipeline only changes when a flag does; the seed changes with
the manifest. The pipeline's two sink urls start with the placeholder
`${WEBHOOK_MANAGER_URL}`: the manager's address contains its hook secret, a
Goldsky sink cannot read its url from a secret, and this repository is public.

To apply, fill the placeholder into a copy git ignores:

```bash
read -rs WEBHOOK_MANAGER_URL && export WEBHOOK_MANAGER_URL   # paste https://<manager>.convex.site/hooks/<hook secret>
indexer/render-pipeline.py --network testnet --fill
goldsky turbo validate indexer/pipelines/testnet.local.yaml
goldsky turbo apply indexer/pipelines/testnet.local.yaml
rm indexer/pipelines/testnet.local.yaml && unset WEBHOOK_MANAGER_URL
```

`WEBHOOK_MANAGER_URL` is the same value the backend deployments hold under that
name; it is in the team's password manager. The filled copy is the committed
file with only the address replaced. Anyone with access to the Goldsky project
can read the address from the pipeline's definition, so that access is as
sensitive as the hook secret.

`--start-at` is a ledger sequence, `earliest` or `latest`, and rendering
requires it. Goldsky keeps the dataset's full history, so a ledger number
backfills from there; 5177900 is just before the first testnet deploy (ledger
5177957, 13 Oct 2026). It is also where a replay starts, so every re-render
passes the value the committed file already has, unless the replay origin is
meant to move.

`--fill` only copies the committed file: it refuses to be combined with a
render flag, and it writes only to a path ending in `.local.yaml`.

`apply` is an upsert: applying a changed YAML under the same `name` keeps the
pipeline's checkpoint and continues from it. Only `restart --clear-state`
replays. Flags of the render script: `--heartbeat-every` (ledgers between
heartbeats, 720), `--resource-size` (`xs`), `--name` (`bwb-stellar-<network>`),
`--secret-name` (`BWB_GOLDSKY_WEBHOOK`), `--out`, and `--fill`.

Goldsky keys the checkpoint on the pipeline's name and the names of its two
sources. Never rename them: a rename is a replay from `start_at`. If
`goldsky turbo list` still shows `error` after an apply, run
`goldsky turbo restart bwb-stellar-testnet`.

## Watch

```bash
goldsky turbo list                         # STATUS column: running, paused, error
goldsky turbo get bwb-stellar-testnet      # the definition as applied
goldsky turbo state list bwb-stellar-testnet   # where each source's checkpoint is
goldsky turbo logs bwb-stellar-testnet     # errors reaching the manager appear here
curl https://<deployment>.convex.site/webhooks/goldsky/health
bunx convex logs                           # from the backend's webhooks-manager/: batches a backend did not take
```

Goldsky only knows whether the manager took a batch. Whether a backend took it
shows in two places: that backend's health route, and the manager's log and
failed deliveries.

The health route returns 200 with `"live": true` while the feed is live, 503
otherwise, with the last heartbeat ledger and time and the last event ledger.
A `live` feed whose last heartbeat ledger is far behind the chain is still
backfilling. The backend logs one error line, `contract event feed is stale`,
when the feed goes stale, and one warning when it comes back.

The health route catches a backend that takes nothing: heartbeats stop landing
and the feed goes stale after two hours. It does not catch a backend that fails
only some batches while heartbeats still land. That shows only on the manager.

The rows live in the `contractEvents` component of the deployment (dashboard:
switch to that component): `events` holds the rows, `deliveries` one row per
batch with the ledger range it covered, which is how to check for gaps after an
incident, and `feeds` the liveness record the health route reports.

## Verify a deploy

1. `goldsky turbo list` shows the pipeline `running`.
2. The health route reports a heartbeat within an hour of the pipeline reaching
   the chain head; during a backfill the heartbeat ledger climbs. The heartbeat
   follows the ledgers stream only: the events stream has caught up when
   `goldsky turbo state list` shows both sources near the chain head.
3. Row count: the component's `events` table holds one row per event the contracts
   emitted in the range. For the first testnet run the count at ledger 5453432
   was 163 events on the 15 contracts the POC filtered, plus the events of the
   three addresses the manifest references only in constructor arguments.
4. Duplicates: `deliveries` rows show `duplicates` only when
   a batch was delivered again, and the sum of `inserted` equals the table's row count.
   `undecodable` should be 0 on every row; a non-zero count is a decoder gap to
   fix in the backend, not a pipeline problem.
5. Checkpoint: re-run `render` with no change and `apply` again; `goldsky turbo
   state list` shows the same positions, and no delivery covers ledgers already
   held.
6. Tracked contracts: with the owner's address in `DATABASE_URL` again, as in
   step 2, `psql "$DATABASE_URL" -c 'SELECT * FROM streamling.testnet_tracked_tokens ORDER BY updated_at'`
   (and `_sales`, `_allowlists`, `_assets`) lists what the pipeline tracks and
   when each row was added.

## When a batch did not land

The manager logs `[delivery] retries exhausted` with the provider `goldsky` and
the listener's name, and keeps the batch seven days.

1. Fix the backend that refused it (its own log has the reason: the token, the
   chain environment, or the write).
2. Replay from the manager, from the backend repository's `webhooks-manager/`:
   `bunx convex run deliveries:replay` (add `--prod` for the production
   manager). It queues every kept failure again, for every provider.
3. Past seven days the batch is gone from the manager. Restart the pipeline
   from a ledger before the gap: re-render with that `--start-at`, fill, apply,
   then `goldsky turbo restart bwb-stellar-testnet --clear-state`. Every
   listener receives the range again and keeps only the rows it lacks.

## Track a new contract

**An offering the backend deploys** needs nothing here. The backend adds the
token, the sale and the allowlist to the tracked-contract tables before it
submits the transaction that creates the token, and again before the sale's.
If the database does not answer, nothing is deployed and the step is tried
again. The pipeline is not touched.

**A contract `scripts/deploy.sh` deploys** is tracked by the seed. Its address
is in the manifest only after the deploy, so its first events have already
passed a pipeline that has caught up:

1. Re-render with the committed file's `--start-at` and commit the new
   `testnet.seed.sql`.
2. Run the seed again, with the owner's address in `DATABASE_URL` as in
   [Setup](#setup), step 2:
   ```bash
   psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f indexer/pipelines/testnet.seed.sql
   ```
   The pipeline tracks the contract from its current position on.
3. Recover what it emitted before that with a replay:
   ```bash
   goldsky turbo restart bwb-stellar-testnet --clear-state
   ```
   It reads again from `start_at`. Every listener keeps the rows it lacks and
   counts the rest as duplicates, so the replay is safe.

**To stop tracking a contract**, delete its row. The pipeline stops delivering
its events within seconds; the rows the backend already holds stay.

## Pause, resume, replay, stop

```bash
goldsky turbo pause bwb-stellar-testnet
goldsky turbo resume bwb-stellar-testnet
goldsky turbo restart bwb-stellar-testnet --clear-state   # replay from start_at
goldsky turbo delete bwb-stellar-testnet
```

A replay redelivers every row to every listener; each holds one row per event
id and reports the rest as duplicates, so a replay is safe and only costs
worker-hours. It is also how a backend deployment that started listening late
gets the events from before it registered. A paused pipeline leaves the
tracked-contract database idle, which stops its compute charge.

## Rotate the token

Turbo pipelines read secrets at worker start. Create the new token, set it on
every listening Convex deployment, update the secret, then
`goldsky turbo restart bwb-stellar-testnet --resync` followed by
`goldsky turbo restart bwb-stellar-testnet`. Batches delivered with the old
token in between are refused with a 401 by the backends, and the manager retries
them and then keeps them; they carry the old token, so do not replay them. If
the gap matters, restart the pipeline from a ledger before it.

If the manager's hook secret is rotated, the address changes: fill and apply
again with the new `WEBHOOK_MANAGER_URL`. Until then Goldsky gets a 401 from the
manager and the pipeline fails, which the apply clears.

## Testnet reset (next: 16 December 2026)

Stellar resets testnet periodically. Every contract and every ledger is gone,
the Goldsky testnet dataset starts over from ledger 1, and the backend's
testnet contract events describe a chain that no longer exists.

1. Pause the pipeline before the reset: `goldsky turbo pause bwb-stellar-testnet`.
2. After the reset, redeploy the contracts with `scripts/deploy.sh`; the
   manifest `deployments/testnet.json` is rewritten with the new ids. Empty the
   tracked-contract tables, whose ids no longer exist:
   `psql "$DATABASE_URL" -c 'TRUNCATE streamling.testnet_tracked_tokens, streamling.testnet_tracked_sales, streamling.testnet_tracked_allowlists, streamling.testnet_tracked_assets'`.
3. On every testnet Convex deployment that keeps contract events, from the
   backend repository's `app/`, run `bunx convex run contractEvents:clearTestnet` (it deletes in batches,
   rescheduling itself until `done: true`; it refuses to run on a mainnet
   deployment). The rows have no value once their contracts
   are gone; nothing else references them.
4. Re-render with `--start-at` just before the first new deploy ledger (or
   `earliest`), commit, run the new seed, fill, validate, then `goldsky turbo apply` and
   `goldsky turbo restart bwb-stellar-testnet --clear-state`. Offerings the
   backend deploys after the reset add themselves.
5. Verify as above. The health route reports live again on the first delivery.

Mainnet never resets; none of this touches the production deployment.

## Mainnet (Tranche 3)

Nothing here is testnet-specific. Mainnet needs, in order: `deployments/mainnet.json`
written by the first mainnet deploy; `CHAIN_ENVIRONMENT=mainnet` and
`GOLDSKY_WEBHOOK_TOKEN` and `CONTRACT_TRACKING_DATABASE_URL` on the production
Convex deployment, with a mainnet token and its own Goldsky secret; then
`render --network mainnet --start-at <ledger before the first mainnet deploy>`,
run `mainnet.seed.sql` (its tables are `streamling.mainnet_tracked_*`, in the
same database), fill with the production manager's `WEBHOOK_MANAGER_URL`,
validate, apply, verify. The second pipeline runs in the same Goldsky project.

The payment asset on mainnet is USDC, which every Stellar application uses:
tracking it delivers every USDC event on the network. Decide before the
mainnet seed whether it belongs in `mainnet_tracked_assets` at all.

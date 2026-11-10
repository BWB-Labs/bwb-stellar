# Contract event indexer

Every event the BWB contracts emit on Stellar is delivered to the backend by a
[Goldsky Turbo](https://docs.goldsky.com/turbo-pipelines) pipeline: Goldsky
reads the chain, filters the events of the contracts in `deployments/<network>.json`
and posts them in batches to the backend's Webhook Manager. The manager queues
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
| `render-pipeline.py` | Renders a pipeline YAML for one network from its deployment manifest, and fills in the manager's address for applying |
| `pipelines/<network>.yaml` | The rendered pipeline, committed so a diff shows what changed. The manager's address is a placeholder |
| `pipelines/<network>.local.yaml` | The filled copy that is applied. It holds a secret and is ignored by git |

## How it works

One pipeline per network, two streams in it, one worker, one checkpoint:

- **Contract events.** `stellar_<network>.events` filtered in SQL to the
  contracts the manifest references: every deployed offering token and sale,
  the single-instance contracts (`offer-allowlist`, the demo payment asset),
  and the addresses that only appear inside constructor arguments (superseded
  allowlists, a token's initial holder). Each row carries the network as
  `chain_environment` and the contract's kind (`token`, `sale`, `allowlist`,
  `asset`) as `contract_kind`, so the backend never guesses the kind from an
  event name two contracts share. Delivered to `/webhooks/goldsky/events`.
- **Heartbeat.** `stellar_<network>.ledgers` filtered to one ledger every 720
  (about one an hour at 5 s per ledger). The contracts are quiet for days, so
  without it a stalled pipeline and an idle chain look the same. Delivered to
  `/webhooks/goldsky/heartbeat`; the backend marks the feed stale when neither a
  heartbeat nor an event has arrived for two hours.

Both sinks send the `Authorization` header from a Goldsky secret, and the
manager passes it on with the body, unchanged.

**What Goldsky sees.** Only the manager. It answers 200 as soon as a batch is
queued, and the pipeline's checkpoint moves then, before any backend holds the
rows. A wrong hook secret in the address answers 401, which fails the pipeline;
a manager that is down is retried forever.

**What the manager sees.** Each backend deployment answers:

| Status | Meaning | Manager |
|---|---|---|
| 200 | batch written (or already held) | done |
| 401 | bad or missing token | retries, then keeps the batch |
| 400 | body is not rows | retries, then keeps the batch |
| 422 | wrong chain environment | retries, then keeps the batch |
| 500 | token not configured, or the write failed | retries, then keeps the batch |

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

1. **Goldsky.** Install the CLI (`curl https://goldsky.com | sh`), `goldsky login`
   with the team's token. The Starter plan runs one pipeline at a time, size
   `xs` or `s`, within 750 free worker-hours a month; `xs` uses half a worker.
   `goldsky project list` shows the active project.
2. **Token.** Generate one and keep it out of the shell history:
   ```bash
   openssl rand -hex 32 > /tmp/goldsky-token && chmod 600 /tmp/goldsky-token
   ```
3. **Goldsky secret**, type `httpauth`, which Goldsky adds as a header to every
   request of a sink that names it:
   ```bash
   goldsky secret create --name BWB_GOLDSKY_WEBHOOK --description "Authorization header for the BWB Convex webhook routes" \
     --value "{\"type\":\"httpauth\",\"secretKey\":\"Authorization\",\"secretValue\":\"Bearer $(cat /tmp/goldsky-token)\"}"
   ```
4. **Backend settings** on every Convex deployment that listens to the manager
   (from the backend repository's `app/`). All listeners of one manager hold the
   same token:
   ```bash
   bunx convex env set GOLDSKY_WEBHOOK_TOKEN "$(cat /tmp/goldsky-token)"
   bunx convex env set CHAIN_ENVIRONMENT testnet
   ```
   `CHAIN_ENVIRONMENT` is the deployment's one chain environment (that
   repository's ADR 0011): `mainnet` on production, `testnet` everywhere else.
   Confirm the routes answer: `curl https://<deployment>.convex.site/webhooks/goldsky/health`
   returns 503 with `"live": false` until the first delivery, which is correct.
   A listener without the token answers 500 to every batch and fills the
   manager's failed deliveries.
5. `rm /tmp/goldsky-token`, and keep the token in the team's password manager:
   every new listener needs it.
6. **Webhook Manager.** The manager of the network's chain environment must
   know the two Goldsky paths (it does once the backend change that adds them is
   pushed to it) and have each backend deployment registered as a listener. Both
   are covered in the backend repository's `webhooks-manager/README.md`;
   previews register themselves.

## Deploy

```bash
indexer/render-pipeline.py --network testnet --start-at 5177900
```

That writes `indexer/pipelines/testnet.yaml`. Commit it with the manifest change
that produced it. Its two sink urls start with the placeholder
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

`--start-at` is a ledger sequence, `earliest` or `latest`. Goldsky keeps the
dataset's full history, so a ledger number backfills from there; 5177900 is
just before the first testnet deploy (ledger 5177957, 13 Oct 2026).

`apply` is an upsert: applying a changed YAML under the same `name` keeps the
pipeline's checkpoint and continues from it. Only `restart --clear-state`
replays. Flags of the render script: `--heartbeat-every` (ledgers between
heartbeats, 720), `--resource-size` (`xs`), `--name` (`bwb-stellar-<network>`),
`--secret-name` (`BWB_GOLDSKY_WEBHOOK`), `--out`, and `--fill`.

## Watch

```bash
goldsky turbo list                         # STATUS column: Running, Paused, Failed
goldsky turbo get bwb-stellar-testnet      # definition and state
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

1. `goldsky turbo list` shows the pipeline `Running`.
2. The health route reports a heartbeat within an hour of the pipeline reaching
   the chain head; during a backfill the heartbeat ledger climbs.
3. Row count: the component's `events` table holds one row per event the contracts
   emitted in the range. For the first testnet run the count at ledger 5453432
   was 163 events on the 15 contracts the POC filtered, plus the events of the
   three addresses the manifest references only in constructor arguments.
4. Duplicates: `deliveries` rows show `duplicates` only when
   a batch was delivered again, and the sum of `inserted` equals the table's row count.
   `undecodable` should be 0 on every row; a non-zero count is a decoder gap to
   fix in the backend, not a pipeline problem.
5. Checkpoint: re-run `render` with no change and `apply` again; `goldsky turbo
   get` shows the same state, and no delivery covers ledgers already held.

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

## Redeploy a contract

After `scripts/deploy.sh` writes the new contract to the manifest: re-render,
commit, fill, validate, apply. The checkpoint is kept; the new contract's events are picked
up from the pipeline's current position, which is at the chain head once the
backfill is done. A contract deployed and used before the pipeline next reads
the chain is still covered: Goldsky reads every ledger in order.

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
gets the events from before it registered. A paused pipeline still counts as the Starter plan's one
pipeline; delete it to run another.

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
   manifest `deployments/testnet.json` is rewritten with the new ids.
3. On every testnet Convex deployment that keeps contract events, from the
   backend repository's `app/`, run `bunx convex run contractEvents:clearTestnet` (it deletes in batches,
   rescheduling itself until `done: true`; it refuses to run on a mainnet
   deployment). The rows have no value once their contracts
   are gone; nothing else references them.
4. Re-render with `--start-at` just before the first new deploy ledger (or
   `earliest`), commit, fill, validate, then `goldsky turbo apply` and
   `goldsky turbo restart bwb-stellar-testnet --clear-state`.
5. Verify as above. The health route reports live again on the first delivery.

Mainnet never resets; none of this touches the production deployment.

## Mainnet (Tranche 3)

Nothing here is testnet-specific. Mainnet needs, in order: `deployments/mainnet.json`
written by the first mainnet deploy; `CHAIN_ENVIRONMENT=mainnet` and
`GOLDSKY_WEBHOOK_TOKEN` on the production Convex deployment, with a mainnet
token and its own Goldsky secret; a second Goldsky pipeline, which on Starter
means a second team or a paid plan, since Starter runs one pipeline at a time;
then `render --network mainnet --start-at <ledger before the first mainnet
deploy>`, fill with the production manager's `WEBHOOK_MANAGER_URL`, validate,
apply, verify.

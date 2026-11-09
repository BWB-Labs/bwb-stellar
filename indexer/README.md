# Contract event indexer

Every event the BWB contracts emit on Stellar is delivered to the backend by a
[Goldsky Turbo](https://docs.goldsky.com/turbo-pipelines) pipeline: Goldsky
reads the chain, filters the events of the contracts in `deployments/<network>.json`
and posts them in batches to two routes of the backend's Convex deployment. The
backend writes each batch before it acknowledges it, keeps one row per Stellar
event id, and treats a batch it could not write as undelivered, so Goldsky
retries and its checkpoint only moves past rows the backend holds. No poller,
no cursor of ours.

This directory holds the pipeline definition and the script that renders it.
The receiver (routes, tables, decoder, liveness cron) lives in the private
backend repository, `app/src/server/contractEvents.ts` and
`model/contractEvents.ts`, with its reasoning in that repository's ADR 0023.

| File | What |
|---|---|
| `render-pipeline.py` | Renders a pipeline YAML for one network from its deployment manifest |
| `pipelines/<network>.yaml` | The rendered pipeline, committed so a diff shows what changed |

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

Both sinks send the `Authorization` header from a Goldsky secret. The backend
rejects a batch whose `chain_environment` is not its own with a 422; Goldsky
fails the pipeline on a 4xx, which is the intended way to notice a pipeline
pointed at the wrong deployment.

The backend answers:

| Status | Meaning | Goldsky |
|---|---|---|
| 200 | batch written (or already held) | moves the checkpoint |
| 401 | bad or missing token | fails the pipeline |
| 400 | body is not rows | fails the pipeline |
| 422 | wrong chain environment | fails the pipeline |
| 500 | token not configured, or the write failed | retries forever |

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
4. **Backend settings** on the Convex deployment that will receive the events
   (from the backend repository's `app/`):
   ```bash
   bunx convex env set GOLDSKY_WEBHOOK_TOKEN "$(cat /tmp/goldsky-token)"
   bunx convex env set CHAIN_ENVIRONMENT testnet
   ```
   `CHAIN_ENVIRONMENT` is the deployment's one chain environment (that
   repository's ADR 0011): `mainnet` on production, `testnet` everywhere else.
   Confirm the routes answer: `curl https://<deployment>.convex.site/webhooks/goldsky/health`
   returns 503 with `"live": false` until the first delivery, which is correct.
5. `rm /tmp/goldsky-token`.

## Deploy

```bash
indexer/render-pipeline.py --network testnet \
  --webhook-base-url https://<deployment>.convex.site \
  --start-at 5177900
goldsky turbo validate indexer/pipelines/testnet.yaml
goldsky turbo apply indexer/pipelines/testnet.yaml
```

`--start-at` is a ledger sequence, `earliest` or `latest`. Goldsky keeps the
dataset's full history, so a ledger number backfills from there; 5177900 is
just before the first testnet deploy (ledger 5177957, 13 Oct 2026). Commit the
rendered YAML with the manifest change that produced it.

`apply` is an upsert: applying a changed YAML under the same `name` keeps the
pipeline's checkpoint and continues from it. Only `restart --clear-state`
replays. Flags of the render script: `--heartbeat-every` (ledgers between
heartbeats, 720), `--resource-size` (`xs`), `--name` (`bwb-stellar-<network>`),
`--secret-name` (`BWB_GOLDSKY_WEBHOOK`), `--out`.

## Watch

```bash
goldsky turbo list                         # STATUS column: Running, Paused, Failed
goldsky turbo get bwb-stellar-testnet      # definition and state
goldsky turbo logs bwb-stellar-testnet     # sink errors appear here
curl https://<deployment>.convex.site/webhooks/goldsky/health
```

The health route returns 200 with `"live": true` while the feed is live, 503
otherwise, with the last heartbeat ledger and time and the last event ledger.
A `live` feed whose last heartbeat ledger is far behind the chain is still
backfilling. The backend logs one error line, `contract event feed is stale`,
when the feed goes stale, and one warning when it comes back.

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
   Goldsky redelivered, and the sum of `inserted` equals the table's row count.
   `undecodable` should be 0 on every row; a non-zero count is a decoder gap to
   fix in the backend, not a pipeline problem.
5. Checkpoint: re-run `render` with no change and `apply` again; `goldsky turbo
   get` shows the same state, and no delivery covers ledgers already held.

## Redeploy a contract

After `scripts/deploy.sh` writes the new contract to the manifest: re-render,
validate, apply. The checkpoint is kept; the new contract's events are picked
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

A replay redelivers every row; the backend holds one row per event id and
reports the rest as duplicates, so a replay is safe and only costs
worker-hours. A paused pipeline still counts as the Starter plan's one
pipeline; delete it to run another.

## Rotate the token

Turbo pipelines read secrets at worker start. Create the new token, set it on
the Convex deployment, update the secret, then
`goldsky turbo restart bwb-stellar-testnet --resync` followed by
`goldsky turbo restart bwb-stellar-testnet`. Deliveries between the Convex
change and the restart get a 401 and fail the pipeline; the restart clears it.

## Testnet reset (next: 16 December 2026)

Stellar resets testnet periodically. Every contract and every ledger is gone,
the Goldsky testnet dataset starts over from ledger 1, and the backend's
testnet contract events describe a chain that no longer exists.

1. Pause the pipeline before the reset: `goldsky turbo pause bwb-stellar-testnet`.
2. After the reset, redeploy the contracts with `scripts/deploy.sh`; the
   manifest `deployments/testnet.json` is rewritten with the new ids.
3. On the testnet Convex deployment, from the backend repository's `app/`,
   run `bunx convex run contractEvents:clearTestnet` (it deletes in batches,
   rescheduling itself until `done: true`; it refuses to run on a mainnet
   deployment). The rows have no value once their contracts
   are gone; nothing else references them.
4. Re-render with `--start-at` just before the first new deploy ledger (or
   `earliest`), validate, then `goldsky turbo apply` and
   `goldsky turbo restart bwb-stellar-testnet --clear-state`.
5. Verify as above. The health route reports live again on the first delivery.

Mainnet never resets; none of this touches the production deployment.

## Mainnet (Tranche 3)

Nothing here is testnet-specific. Mainnet needs, in order: `deployments/mainnet.json`
written by the first mainnet deploy; `CHAIN_ENVIRONMENT=mainnet` and
`GOLDSKY_WEBHOOK_TOKEN` on the production Convex deployment, with a mainnet
token and its own Goldsky secret; a second Goldsky pipeline, which on Starter
means a second team or a paid plan, since Starter runs one pipeline at a time;
then `render --network mainnet --webhook-base-url <production site URL>
--start-at <ledger before the first mainnet deploy>`, validate, apply, verify.

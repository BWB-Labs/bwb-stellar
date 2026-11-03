# Goldsky POC: Stellar contract events, delivered by webhook

Goal: prove that a Goldsky **Turbo pipeline** can read our offering contracts'
events on Stellar testnet, backfill from the first deploy, and push them to us
through a webhook. Two receivers exist: a file on this machine (no accounts
beyond Goldsky) and a Convex deployment (the shape we would ship).

Goldsky retired "Mirror" pipelines; Stellar datasets exist only in Turbo, so
every command below is `goldsky turbo ...`, not `goldsky pipeline ...`.

## What is in this folder

| Path | What it is |
|---|---|
| `render-pipeline.py` | Reads `deployments/testnet.json` and writes the pipeline YAML with every deployed contract in the SQL filter. Re-run after each deploy. |
| `pipeline.testnet.yaml` | The rendered pipeline (15 contracts, backfill from ledger 5177900, the first deploy on 13 Oct). Generated; do not edit by hand. |
| `receiver-file/server.mjs` | Path A. A Node server that appends every event as one JSON line to `events.jsonl` and logs each delivery. No dependencies. Tested locally. |
| `receiver/` | Path B. A minimal Convex project: an HTTP action at `/goldsky` that checks a bearer token, dedupes on the event id and stores each event plus a delivery log. Typechecks; not deployed. |

## Path A: events into a text file (no Convex)

### A1. Goldsky account and CLI (you)

1. Sign up at https://app.goldsky.com (Google, GitHub or email). Starter gives
   a one-time $100 credit, no card. An `xs` pipeline burns about $36/month of
   it, so the POC is covered.
2. Install the CLI and the Turbo extension:

```bash
curl https://goldsky.com | sh
```

```bash
curl https://install-turbo.goldsky.com | sh
```

3. Log in (browser flow) and confirm the project is empty:

```bash
goldsky login
```

```bash
goldsky turbo list
```

### A2. Start the receiver and expose it

Terminal 1, the receiver (writes to `poc/goldsky/receiver-file/events.jsonl`):

```bash
node poc/goldsky/receiver-file/server.mjs
```

Terminal 2, a public HTTPS URL for port 8787. Tailscale is installed on this
machine but its daemon was not running on 3 Nov; open the Tailscale app first.
Funnel gives a stable `https://<machine>.<tailnet>.ts.net` URL (it must be
enabled once for the tailnet, the command tells you if not):

```bash
tailscale funnel 8787
```

Fallback if Funnel is not allowed on the tailnet, a throwaway Cloudflare URL:

```bash
brew install cloudflared && cloudflared tunnel --url http://localhost:8787
```

Check from outside: `curl https://<public-url>/` should return `{"ok":true,"events":0}`.

### A3. Render, validate, apply

```bash
python3 poc/goldsky/render-pipeline.py --start-at 5177900 --no-secret --webhook-url https://<public-url>/goldsky
```

```bash
goldsky turbo validate poc/goldsky/pipeline.testnet.yaml
```

```bash
goldsky turbo apply poc/goldsky/pipeline.testnet.yaml
```

If `validate` says the webhook sink needs a secret, create one and drop
`--no-secret`: `goldsky secret create BWB_WEBHOOK_SECRET` (type httpauth,
header `Authorization`, value `Bearer <anything>`), and start the receiver
with `GOLDSKY_WEBHOOK_TOKEN=<anything>`.

### A4. Watch

```bash
goldsky turbo logs bwb-stellar-events-testnet -f
```

```bash
tail -f poc/goldsky/receiver-file/deliveries.log
```

Then read the data:

```bash
wc -l poc/goldsky/receiver-file/events.jsonl
```

```bash
python3 -c "import json,collections;c=collections.Counter(json.loads(l)['contract_id'] for l in open('poc/goldsky/receiver-file/events.jsonl'));print(*c.most_common(),sep='\n')"
```

```bash
head -n 3 poc/goldsky/receiver-file/events.jsonl | python3 -m json.tool
```

The last one shows how Goldsky encodes `topics` and `data`, which the docs do
not say.

## Path B: events into Convex

Same pipeline, different URL and a secret.

1. In `poc/goldsky/receiver`: `npm install && npx convex dev` (browser login,
   create project `bwb-goldsky-poc`). Then
   `npx convex env set GOLDSKY_WEBHOOK_TOKEN "$(openssl rand -hex 32)"`.
2. `curl https://<deployment>.convex.site/goldsky` returns `{"ok":true,...}`.
3. `goldsky secret create BWB_WEBHOOK_SECRET`: type httpauth, header
   `Authorization`, value `Bearer <that token>`.
4. Render without `--no-secret` and with the Convex URL, validate, apply.
5. In the Convex dashboard run `events:stats` (totals, per contract, last 20
   deliveries) and `events:list`.

Applying the same pipeline name with the new URL is an upsert that keeps the
checkpoint, so Path B can continue where Path A stopped. To replay the backfill
into Convex, use a new pipeline name or `goldsky turbo restart <name> --clear-state`.

## What the POC should answer

- Does the backfill from ledger 5177900 deliver every event our contracts
  emitted since 13 Oct? Compare counts with stellar.expert per contract.
- What is the encoding of `topics` and `data`: decoded ScVal JSON or XDR?
- Batch sizes and duplicates under normal flow (at-least-once delivery).
- Does `goldsky turbo apply` with one more contract in the list keep the
  checkpoint? (Docs say yes.)
- What happens when the receiver is down for a minute: retries, then the same
  batch again? Stop the receiver, watch the pipeline logs, start it again.
- How long until the pipeline catches up with the chain head, on `xs`.

## Operating notes

- Goldsky retries 5xx, 408 and 429 forever with backoff; any other 4xx fails
  the pipeline. Both receivers return 401 only for a wrong token (stop and fix)
  and 500 when the write fails (retry).
- Delivery is at least once and ordering across restarts is not guaranteed.
  The event id is the dedupe key; sort by `ledger_sequence` when reading.
- To add a contract: redeploy, re-run `render-pipeline.py`, `goldsky turbo
  apply`. To replay history for a contract added late, rename the source node
  or `goldsky turbo restart <name> --clear-state`.
- Testnet resets on 16 Dec 2026. After it, redeploy, re-render with the new
  first ledger, and apply under a new pipeline name.
- Pause and delete:

```bash
goldsky turbo pause bwb-stellar-events-testnet
```

```bash
goldsky turbo delete bwb-stellar-events-testnet
```

Sources: Goldsky docs read on 3 Nov 2026: turbo-pipelines/cli, pipeline-config,
sources/stellar, sinks/webhook, delivery-guarantees, platform/secrets,
pricing/summary, chains/stellar.

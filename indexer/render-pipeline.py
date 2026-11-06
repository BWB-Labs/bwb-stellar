#!/usr/bin/env python3
"""Render the Goldsky Turbo pipeline for one network from deployments/<network>.json.

    indexer/render-pipeline.py --network testnet --webhook-base-url https://<deployment>.convex.site \
        [--start-at latest|earliest|<ledger>] [--heartbeat-every 720] [--resource-size xs] [--out <file>]

The pipeline has two streams that share one worker and one checkpoint:

- contract events of every contract the manifest references, filtered in SQL
  and delivered in batches to <base>/webhooks/goldsky/events;
- a heartbeat, one ledger row every --heartbeat-every ledgers, delivered to
  <base>/webhooks/goldsky/heartbeat so the receiver can tell a stalled pipeline
  from a quiet one.

Every row carries its chain environment (the network) and, for events, the
contract's kind from the manifest; the receiver rejects rows for any other
chain environment. Re-run after each deploy and `goldsky turbo apply` the result:
apply is an upsert and keeps the pipeline's checkpoint. See indexer/README.md.
"""
import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
DATASET_VERSION = "1.2.0"
CONTRACT_ID = re.compile(r"\bC[A-Z2-7]{55}\b")
EVENTS_PATH = "/webhooks/goldsky/events"
HEARTBEAT_PATH = "/webhooks/goldsky/heartbeat"


def parse_args():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--network", default="testnet", help="deployments/<network>.json to read (default: testnet)")
    ap.add_argument("--start-at", default="latest",
                    help="latest | earliest | <ledger sequence>. The dataset keeps full history, so a ledger number backfills.")
    ap.add_argument("--webhook-base-url", required=True,
                    help="the Convex deployment's site URL, e.g. https://<name>.convex.site")
    ap.add_argument("--secret-name", default="BWB_GOLDSKY_WEBHOOK",
                    help="Goldsky secret holding the Authorization header (default: BWB_GOLDSKY_WEBHOOK)")
    ap.add_argument("--heartbeat-every", type=int, default=720,
                    help="deliver one ledger row every N ledgers; 720 is about one an hour (default: 720)")
    ap.add_argument("--resource-size", default="xs", help="Goldsky worker size (default: xs)")
    ap.add_argument("--name", default=None, help="pipeline name (default: bwb-stellar-<network>)")
    ap.add_argument("--out", default=None, help="output file (default: indexer/pipelines/<network>.yaml)")
    return ap.parse_args()


KIND_BY_CRATE = {"offer-token": "token", "offer-sale": "sale", "offer-allowlist": "allowlist"}
KIND_BY_CONSTRUCTOR_KEY = {"initial_holder": "sale", "token": "token", "payment_asset": "asset", "allowlist": "allowlist"}


def kind_of(entry):
    """The contract kind the receiver stores: token, sale, allowlist or asset."""
    if entry.get("crate") in KIND_BY_CRATE:
        return KIND_BY_CRATE[entry["crate"]]
    if entry.get("asset"):
        return "asset"
    return None


def referenced_contracts(constructor_args):
    """(key, contract id) pairs from constructor arguments, by argument name or JSON key."""
    pairs = []
    for flag, value in zip(constructor_args, constructor_args[1:]):
        if not flag.startswith("--"):
            continue
        key = flag[2:]
        if CONTRACT_ID.fullmatch(value):
            pairs.append((key, value))
            continue
        try:
            parsed = json.loads(value)
        except ValueError:
            continue
        if isinstance(parsed, dict):
            pairs.extend((k, v) for k, v in parsed.items() if isinstance(v, str) and CONTRACT_ID.fullmatch(v))
    return pairs


def collect_contracts(manifest):
    """Every contract id the manifest references, as (id, kind, label), in a stable order.

    Deployed offering contracts and the single-instance contracts come first.
    Addresses that only appear inside constructor arguments (superseded
    allowlists, a token's initial holder) come after: they still emit events
    the backend may read, and one more id in the filter costs nothing. Each id
    gets the kind the receiver stores, so the receiver never has to guess it
    from the event's name.
    """
    seen = {}

    def add(contract_id, kind, label):
        if contract_id and contract_id not in seen:
            seen[contract_id] = (kind, label)

    for offering, parts in manifest.get("offerings", {}).items():
        for part, contract in parts.items():
            if contract.get("status") == "deployed":
                kind = kind_of(contract)
                if kind is None:
                    sys.exit(f"{offering} {part}: cannot tell the contract kind (no known crate)")
                add(contract.get("contract_id"), kind, f"{offering} {part}")
    for name, contract in manifest.get("contracts", {}).items():
        kind = kind_of(contract)
        if kind is None:
            sys.exit(f"contracts.{name}: cannot tell the contract kind (no known crate or asset)")
        add(contract.get("contract_id"), kind, name)
    for offering, parts in manifest.get("offerings", {}).items():
        for part, contract in parts.items():
            if contract.get("status") != "deployed":
                continue
            for key, contract_id in referenced_contracts(contract.get("constructor_args", [])):
                kind = KIND_BY_CONSTRUCTOR_KEY.get(key)
                if kind is None:
                    print(f"warning: {offering} {part} references {contract_id} as {key!r}, kind unknown, skipped", file=sys.stderr)
                    continue
                add(contract_id, kind, f"{offering} {part} {key}")
    return [(contract_id, kind, label) for contract_id, (kind, label) in seen.items()]


def render(args, contracts):
    ids_sql = "\n".join(
        f"          '{contract_id}'{',' if i < len(contracts) - 1 else ''}  -- {label}"
        for i, (contract_id, _kind, label) in enumerate(contracts)
    )
    kinds_sql = "\n".join(
        f"          WHEN '{contract_id}' THEN '{kind}'"
        for contract_id, kind, _label in contracts
    )
    name = args.name or f"bwb-stellar-{args.network}"
    base = args.webhook_base_url.rstrip("/")
    return f"""# Generated by indexer/render-pipeline.py from deployments/{args.network}.json. Do not edit by hand.
# Re-render after every deploy, then `goldsky turbo apply` this file (see indexer/README.md).
name: {name}
description: BWB contract events and liveness heartbeat on Stellar {args.network}, delivered to Convex
resource_size: {args.resource_size}
sources:
  stellar_events:
    type: dataset
    dataset_name: stellar_{args.network}.events
    version: {DATASET_VERSION}
    start_at: {args.start_at}
    fetch_parallelism: 2
  stellar_ledgers:
    type: dataset
    dataset_name: stellar_{args.network}.ledgers
    version: {DATASET_VERSION}
    start_at: {args.start_at}
    fetch_parallelism: 2
transforms:
  bwb_contract_events:
    type: sql
    primary_key: id
    sql: |
      SELECT
        id, contract_id, topics, data,
        transaction_hash, transaction_successful, in_successful_contract_call,
        ledger_sequence, ledger_closed_at,
        '{args.network}' AS chain_environment,
        CASE contract_id
{kinds_sql}
        END AS contract_kind
      FROM stellar_events
      WHERE type = 'contract'
        AND contract_id IN (
{ids_sql}
        )
  heartbeat:
    type: sql
    primary_key: ledger_sequence
    sql: |
      SELECT
        ledger_sequence, ledger_closed_at,
        '{args.network}' AS chain_environment
      FROM stellar_ledgers
      WHERE ledger_sequence % {args.heartbeat_every} = 0
sinks:
  convex_events:
    type: webhook
    from: bwb_contract_events
    url: {base}{EVENTS_PATH}
    secret_name: {args.secret_name}
    one_row_per_request: false
    batch_size: 100
    batch_flush_interval: 1s
  convex_heartbeat:
    type: webhook
    from: heartbeat
    url: {base}{HEARTBEAT_PATH}
    secret_name: {args.secret_name}
    one_row_per_request: false
    batch_size: 10
    batch_flush_interval: 1s
"""


def main():
    args = parse_args()
    manifest_path = ROOT / "deployments" / f"{args.network}.json"
    if not manifest_path.exists():
        sys.exit(f"no manifest at {manifest_path.relative_to(ROOT)}")
    manifest = json.load(open(manifest_path))
    if manifest.get("network") != args.network:
        sys.exit(f"{manifest_path.name} says network={manifest.get('network')!r}, not {args.network!r}")
    contracts = collect_contracts(manifest)
    if not contracts:
        sys.exit("no deployed contracts found")
    out = pathlib.Path(args.out) if args.out else ROOT / "indexer" / "pipelines" / f"{args.network}.yaml"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(render(args, contracts))
    shown = out.relative_to(ROOT) if out.is_relative_to(ROOT) else out
    print(f"wrote {shown}: {len(contracts)} contracts, start_at={args.start_at}, heartbeat every {args.heartbeat_every} ledgers")


if __name__ == "__main__":
    main()

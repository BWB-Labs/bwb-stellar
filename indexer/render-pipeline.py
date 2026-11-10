#!/usr/bin/env python3
"""Render the Goldsky Turbo pipeline for one network from deployments/<network>.json.

    indexer/render-pipeline.py --network testnet --start-at latest|earliest|<ledger> \
        [--heartbeat-every 720] [--resource-size xs] [--out <file>]
    WEBHOOK_MANAGER_URL=https://<manager>.convex.site/hooks/<hook secret> \
        indexer/render-pipeline.py --network testnet --fill

The pipeline has two streams that share one worker and one checkpoint:

- contract events of every contract the manifest references, filtered in SQL
  and delivered in batches to <manager>/webhooks/goldsky/events;
- a heartbeat, one ledger row every --heartbeat-every ledgers, delivered to
  <manager>/webhooks/goldsky/heartbeat so a receiver can tell a stalled pipeline
  from a quiet one.

Both are addressed to the backend's Webhook Manager, which hands each batch to
every backend deployment listening to it. The manager's address contains its
hook secret and Goldsky cannot read a sink's url from a secret, so the rendered
file carries a placeholder and is safe to commit. --fill copies the committed
file to pipelines/<network>.local.yaml (ignored by git) with the address from
WEBHOOK_MANAGER_URL in place of the placeholder; that copy is what
`goldsky turbo apply` takes.

Every row carries its chain environment (the network) and, for events, the
contract's kind from the manifest; a receiver rejects rows for any other
chain environment. Re-run after each deploy, fill and apply: apply is an upsert
and keeps the pipeline's checkpoint. See indexer/README.md.
"""
import argparse
import json
import os
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
DATASET_VERSION = "1.2.0"
CONTRACT_ID = re.compile(r"\bC[A-Z2-7]{55}\b")
LABEL = re.compile(r"[A-Za-z0-9_. -]+")
EVENTS_PATH = "/webhooks/goldsky/events"
HEARTBEAT_PATH = "/webhooks/goldsky/heartbeat"
MANAGER_URL_VARIABLE = "WEBHOOK_MANAGER_URL"
MANAGER_URL_PLACEHOLDER = "${" + MANAGER_URL_VARIABLE + "}"
MANAGER_URL = re.compile(r"https://[a-z0-9-]+\.convex\.site/hooks/[A-Za-z0-9_-]+")


RENDER_ONLY = ("start_at", "secret_name", "heartbeat_every", "resource_size", "name")


def parse_args():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--network", default="testnet", help="deployments/<network>.json to read (default: testnet)")
    ap.add_argument("--start-at", default=None,
                    help="latest | earliest | <ledger sequence>, required to render. The dataset keeps full history, "
                         "so a ledger number backfills. It is where a replay starts, so re-render with the value "
                         "the committed file already has unless the replay origin is meant to move.")
    ap.add_argument("--fill", action="store_true",
                    help=f"write pipelines/<network>.local.yaml from the committed file, with ${MANAGER_URL_VARIABLE} "
                         "in place of the placeholder; this is the file to apply")
    ap.add_argument("--secret-name", default="BWB_GOLDSKY_WEBHOOK",
                    help="Goldsky secret holding the Authorization header (default: BWB_GOLDSKY_WEBHOOK)")
    ap.add_argument("--heartbeat-every", type=int, default=720,
                    help="deliver one ledger row every N ledgers; 720 is about one an hour (default: 720)")
    ap.add_argument("--resource-size", default="xs", help="Goldsky worker size (default: xs)")
    ap.add_argument("--name", default=None, help="pipeline name (default: bwb-stellar-<network>)")
    ap.add_argument("--out", default=None,
                    help="output file (default: indexer/pipelines/<network>.yaml, or <network>.local.yaml with --fill, "
                         "where it must end in .local.yaml so git ignores it)")
    args = ap.parse_args()
    if args.fill:
        ignored = [f"--{name.replace('_', '-')}" for name in RENDER_ONLY if getattr(args, name) != ap.get_default(name)]
        if ignored:
            ap.error(f"--fill copies the committed file as it is and would ignore {', '.join(ignored)}; render first, then fill")
        if args.out and not args.out.endswith(".local.yaml"):
            ap.error("--fill writes the hook secret: --out must end in .local.yaml, which git ignores")
    elif args.start_at is None:
        ap.error("--start-at is required to render (latest, earliest or a ledger sequence)")
    return args


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
        if not contract_id or contract_id in seen:
            return
        if not CONTRACT_ID.fullmatch(contract_id):
            sys.exit(f"{label}: {contract_id!r} is not a contract id")
        if not LABEL.fullmatch(label):
            sys.exit(f"manifest key {label!r} has characters the rendered YAML cannot carry")
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
    return f"""# Generated by indexer/render-pipeline.py from deployments/{args.network}.json. Do not edit by hand.
# Re-render after every deploy. The sink urls carry a placeholder for the Webhook Manager's address,
# which contains a secret: `render-pipeline.py --fill` writes the copy to apply (see indexer/README.md).
name: {name}
description: BWB contract events and liveness heartbeat on Stellar {args.network}, delivered to the Webhook Manager
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
    url: {MANAGER_URL_PLACEHOLDER}{EVENTS_PATH}
    secret_name: {args.secret_name}
    one_row_per_request: false
    batch_size: 100
    batch_flush_interval: 1s
  convex_heartbeat:
    type: webhook
    from: heartbeat
    url: {MANAGER_URL_PLACEHOLDER}{HEARTBEAT_PATH}
    secret_name: {args.secret_name}
    one_row_per_request: false
    batch_size: 10
    batch_flush_interval: 1s
"""


def fill(args):
    """Write the file to apply: the committed pipeline with the manager's address filled in."""
    committed = ROOT / "indexer" / "pipelines" / f"{args.network}.yaml"
    if not committed.exists():
        sys.exit(f"no rendered pipeline at {committed.relative_to(ROOT)}; render it first")
    rendered = committed.read_text()
    if MANAGER_URL_PLACEHOLDER not in rendered:
        sys.exit(f"{committed.relative_to(ROOT)} has no {MANAGER_URL_PLACEHOLDER} placeholder; render it again")
    manager_url = os.environ.get(MANAGER_URL_VARIABLE, "").rstrip("/")
    if not MANAGER_URL.fullmatch(manager_url):
        sys.exit(f"{MANAGER_URL_VARIABLE} must be https://<manager>.convex.site/hooks/<hook secret>")
    out = pathlib.Path(args.out) if args.out else committed.with_suffix(".local.yaml")
    out.touch(mode=0o600)
    out.chmod(0o600)
    out.write_text(rendered.replace(MANAGER_URL_PLACEHOLDER, manager_url))
    shown = out.relative_to(ROOT) if out.is_relative_to(ROOT) else out
    print(f"wrote {shown} for `goldsky turbo apply`; it holds the hook secret, do not commit or share it")


def main():
    args = parse_args()
    if args.fill:
        return fill(args)
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

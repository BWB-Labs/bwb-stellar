#!/usr/bin/env python3
"""Render the Goldsky Turbo pipeline for one network, and the SQL that seeds its tracked contracts.

    indexer/render-pipeline.py --network testnet --start-at latest|earliest|<ledger sequence> \
        [--heartbeat-every 720] [--resource-size xs] [--out <file>]
    WEBHOOK_MANAGER_URL=https://<manager>.convex.site/hooks/<hook secret> \
        indexer/render-pipeline.py --network testnet --fill

The pipeline has two streams that share one worker and one checkpoint:

- contract events of every tracked contract, delivered in batches to
  <manager>/webhooks/goldsky/events;
- a heartbeat, one ledger row every --heartbeat-every ledgers, delivered to
  <manager>/webhooks/goldsky/heartbeat so a receiver can tell a stalled pipeline
  from a quiet one.

The tracked contracts are not in the pipeline. They are rows in four Postgres
tables, one per contract kind, that the pipeline consults on every batch
(Goldsky dynamic tables). A row added while the pipeline runs takes effect
within seconds, with no apply and no restart: the backend adds the contracts it
deploys, before it deploys them. Rendering also writes
pipelines/<network>.seed.sql, which creates the four tables, adds every
contract deployments/<network>.json references and lets the backend's database
user add rows; it is safe to run again.

Both streams are addressed to the backend's Webhook Manager, which hands each
batch to every backend deployment listening to it. The manager's address
contains its hook secret and Goldsky cannot read a sink's url from a secret, so
the rendered file carries a placeholder and is safe to commit. --fill copies
the committed file to pipelines/<network>.local.yaml (ignored by git) with the
address from WEBHOOK_MANAGER_URL in place of the placeholder; that copy is what
`goldsky turbo apply` takes.

Every row carries its chain id (the SHA-256 of the network passphrase in the
manifest, which is Stellar's network id) and, for events, the contract's kind,
from the table that tracks it; a receiver rejects rows for any other chain. See
indexer/README.md.
"""
import argparse
import hashlib
import json
import os
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
DATASET_VERSION = "1.2.0"
CONTRACT_ID = re.compile(r"\bC[A-Z2-7]{55}\b")
LABEL = re.compile(r"[A-Za-z0-9_. -]+")
NETWORK = re.compile(r"[a-z][a-z0-9_]*")
START_AT = re.compile(r"latest|earliest|\d+")
NAME = re.compile(r"[A-Za-z0-9_-]+")
EVENTS_PATH = "/webhooks/goldsky/events"
HEARTBEAT_PATH = "/webhooks/goldsky/heartbeat"
MANAGER_URL_VARIABLE = "WEBHOOK_MANAGER_URL"
MANAGER_URL_PLACEHOLDER = "${" + MANAGER_URL_VARIABLE + "}"
MANAGER_URL = re.compile(r"https://[a-z0-9-]+\.convex\.site/hooks/[A-Za-z0-9_-]+")


RENDER_ONLY = ("start_at", "secret_name", "heartbeat_every", "resource_size", "name")
KINDS = ("token", "sale", "allowlist", "asset")
TABLES_SCHEMA = "streamling"
TABLES_SECRET = "BWB_INDEXER_POSTGRES"
BACKEND_ROLE = "bwb_backend"


def parse_args():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--network", default="testnet", help="network to render; deployments/<network>.json seeds it (default: testnet)")
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
    else:
        if not START_AT.fullmatch(args.start_at):
            ap.error("--start-at must be latest, earliest or a ledger sequence")
        if args.heartbeat_every < 1:
            ap.error("--heartbeat-every must be at least 1")
        for flag in ("secret_name", "resource_size", "name"):
            value = getattr(args, flag)
            if value is not None and not NAME.fullmatch(value):
                ap.error(f"--{flag.replace('_', '-')} must be letters, digits, hyphens or underscores")
    if not NETWORK.fullmatch(args.network):
        ap.error("--network must be lowercase letters, digits or underscores: it names the tracked-contract tables")
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
    the backend may read, and one more tracked id costs nothing. Each id gets
    the kind the receiver stores, so the receiver never has to guess it from
    the event's name.
    """
    seen = {}

    def add(contract_id, kind, label):
        if not contract_id:
            sys.exit(f"{label}: no contract_id in the manifest")
        if contract_id in seen:
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


def transform_of(kind):
    """The dynamic table transform that tracks one contract kind."""
    return f"tracked_{kind}s"


def table_of(network, kind):
    """The Postgres table behind it. One database can hold every network's tables."""
    return f"{network}_tracked_{kind}s"


def chain_id_of(manifest):
    """Stellar's network id: the SHA-256 of the network passphrase, the id the backend knows the chain by."""
    passphrase = manifest.get("passphrase")
    if not isinstance(passphrase, str) or not passphrase:
        sys.exit("the manifest has no passphrase to derive the chain id from")
    return hashlib.sha256(passphrase.encode()).hexdigest()


def render(args, chain_id):
    tables_yaml = "\n".join(
        f"""  {transform_of(kind)}:
    type: dynamic_table
    backend_type: Postgres
    backend_entity_name: {table_of(args.network, kind)}
    secret_name: {TABLES_SECRET}
    max_connections: 2"""
        for kind in KINDS
    )
    kinds_sql = "\n".join(
        f"          WHEN dynamic_table_check('{transform_of(kind)}', contract_id) THEN '{kind}'"
        for kind in KINDS
    )
    tracked_sql = "\n".join(
        f"          {'OR ' if i else ''}dynamic_table_check('{transform_of(kind)}', contract_id)"
        for i, kind in enumerate(KINDS)
    )
    name = args.name or f"bwb-stellar-{args.network}"
    return f"""# Generated by indexer/render-pipeline.py. Do not edit by hand.
# The tracked contracts are rows in the {TABLES_SCHEMA}.{args.network}_tracked_* tables, not in this file.
# The sink urls carry a placeholder for the Webhook Manager's address, which contains a secret:
# `render-pipeline.py --fill` writes the copy to apply (see indexer/README.md).
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
{tables_yaml}
  bwb_contract_events:
    type: sql
    primary_key: id
    sql: |
      SELECT
        id, contract_id, topics, data,
        transaction_hash, transaction_successful, in_successful_contract_call,
        ledger_sequence, ledger_closed_at,
        '{chain_id}' AS chain_id,
        CASE
{kinds_sql}
        END AS contract_kind
      FROM stellar_events
      WHERE type = 'contract'
        AND (
{tracked_sql}
        )
  heartbeat:
    type: sql
    primary_key: ledger_sequence
    sql: |
      SELECT
        ledger_sequence, ledger_closed_at,
        '{chain_id}' AS chain_id
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


def render_seed(network, contracts):
    """SQL that creates the tracked-contract tables and adds the manifest's contracts.

    The tables have the shape Goldsky gives a dynamic table it creates itself,
    so creating them first changes nothing for the pipeline, and it lets the
    contracts be there before the pipeline reads its first ledger. Contract ids
    are stored as Stellar writes them: the lookup is an exact match. The
    backend's database user may add rows and nothing else.
    """
    statements = [
        f"-- Generated by indexer/render-pipeline.py from deployments/{network}.json. Do not edit by hand.",
        "-- Creates the tracked-contract tables and adds the manifest's contracts. Safe to run again.",
        f"CREATE SCHEMA IF NOT EXISTS {TABLES_SCHEMA};",
    ]
    tables = {kind: f"{TABLES_SCHEMA}.{table_of(network, kind)}" for kind in KINDS}
    for kind, table in tables.items():
        statements.append(
            f"CREATE TABLE IF NOT EXISTS {table} (\n"
            "  value text PRIMARY KEY,\n"
            "  updated_at timestamptz NOT NULL DEFAULT clock_timestamp()\n"
            ");"
        )
        rows = [(contract_id, label) for contract_id, contract_kind, label in contracts if contract_kind == kind]
        if not rows:
            continue
        values = "\n".join(
            f"  ('{contract_id}'){',' if i < len(rows) - 1 else ''}  -- {label}"
            for i, (contract_id, label) in enumerate(rows)
        )
        statements.append(f"INSERT INTO {table} (value) VALUES\n{values}\nON CONFLICT (value) DO NOTHING;")
    statements.append(f"GRANT USAGE ON SCHEMA {TABLES_SCHEMA} TO {BACKEND_ROLE};")
    statements.append(f"GRANT INSERT ON {", ".join(tables.values())} TO {BACKEND_ROLE};")
    return "\n".join(statements) + "\n"


def shown(path):
    """The path as printed: relative to the repository when it is inside it."""
    return path.relative_to(ROOT) if path.is_relative_to(ROOT) else path


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
    print(f"wrote {shown(out)} for `goldsky turbo apply`; it holds the hook secret, do not commit or share it")


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
    out.write_text(render(args, chain_id_of(manifest)))
    seed = out.with_suffix(".seed.sql")
    seed.write_text(render_seed(args.network, contracts))
    print(f"wrote {shown(out)}: start_at={args.start_at}, heartbeat every {args.heartbeat_every} ledgers")
    print(f"wrote {shown(seed)}: {len(contracts)} contracts to track")


if __name__ == "__main__":
    main()

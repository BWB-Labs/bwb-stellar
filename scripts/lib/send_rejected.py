#!/usr/bin/env python3
"""Submit a contract call that is expected to fail, so the rejection is
recorded on the ledger with its error code.

The Stellar CLI simulates every call before sending it and stops when the
simulation fails, so a rejected call never gets a transaction hash. This
script skips simulation: it takes the unsimulated envelope from
`stellar contract invoke --build-only`, adds the authorization entry and
the resources by hand, then signs and sends it. The network executes it,
it fails with the contract's error, and the fee is still charged.

    stellar contract invoke --build-only ... | \\
      scripts/lib/send_rejected.py --network testnet --source <identity> \\
        --wasm-hash <hex> --expect <code> [--key '<ScVal json>' ...]
        [--template-file <simulated envelope>] [--swap G...:G...] \\
        [--payment-sub <asset contract>]

Two ways to declare the footprint (the ledger entries the call reads and
writes before it fails):

- `--key`: a persistent storage key of the called contract, repeatable.
  The contract instance and code are always in the footprint. Enough for
  a call that touches only its own entries.
- `--template-file`: the base64 envelope of a *passing* twin of the call,
  simulated with `stellar tx simulate`. Its footprint and resources are
  reused, which covers calls that reach other contracts and classic
  entries (a purchase touches the payment asset, the allowlist, the
  investor's account and trustline). `--swap FROM:TO` rewrites an account
  in that footprint, so the twin may be another investor's call.

`--payment-sub <contract>` adds, under the root authorization, the
`transfer(arg0, called contract, arg1)` sub-invocation the sale makes on
the payment asset in the investor's name, so a purchase gets past the
asset's own authorization check and fails where it should.

Prints the transaction hash. Exits 0 only when the transaction was
included and failed with contract error --expect; a call that succeeds or
fails for another reason (a missing footprint entry, say) is an error.
"""
import argparse
import json
import subprocess
import sys

RESOURCE_FEE = 5_000_000  # stroops (0.5 XLM), generous: testnet only
INSTRUCTIONS = 10_000_000


def stellar(*args, stdin=""):
    proc = subprocess.run(["stellar", *args], input=stdin, capture_output=True, text=True)
    return proc.returncode, proc.stdout.strip(), proc.stderr.strip()


def decode(xdr):
    code, out, err = stellar("tx", "decode", stdin=xdr)
    if code:
        sys.exit(f"cannot decode envelope: {err}")
    return json.loads(out)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--network", required=True)
    p.add_argument("--source", required=True)
    p.add_argument("--wasm-hash", required=True)
    p.add_argument("--expect", required=True, type=int, help="expected contract error code")
    p.add_argument("--key", action="append", default=[])
    p.add_argument("--template-file", help="a simulated passing twin whose footprint is reused")
    p.add_argument("--swap", action="append", default=[], help="FROM:TO, an account to rewrite in the template")
    p.add_argument("--payment-sub", help="the payment asset: adds the investor's transfer sub-invocation")
    a = p.parse_args()

    env = decode(sys.stdin.read())
    tx = env["tx"]["tx"]
    op = tx["operations"][0]["body"]["invoke_host_function"]
    call = op["host_function"]["invoke_contract"]
    contract = call["contract_address"]

    # The source account authorizes the root call, as recording simulation
    # would do for a require_auth on the source, and the payment transfer
    # the sale makes in its name, when asked.
    sub_invocations = []
    if a.payment_sub:
        sub_invocations.append({
            "function": {"contract_fn": {
                "contract_address": a.payment_sub,
                "function_name": "transfer",
                "args": [call["args"][0], {"address": contract}, call["args"][1]],
            }},
            "sub_invocations": [],
        })
    op["auth"] = [{
        "credentials": "source_account",
        "root_invocation": {"function": {"contract_fn": call}, "sub_invocations": sub_invocations},
    }]

    if a.template_file:
        template = decode(open(a.template_file).read().strip())
        resources = template["tx"]["tx"]["ext"]["v1"]["resources"]
        text = json.dumps(resources)
        for swap in a.swap:
            src, dst = swap.split(":")
            text = text.replace(src, dst)
        resources = json.loads(text)
        # The twin passed; the rejection reads at most what it read. Double
        # the budget so a different branch still fits.
        resources["instructions"] = max(2 * resources["instructions"], INSTRUCTIONS)
        resources["disk_read_bytes"] = max(2 * resources["disk_read_bytes"], 20_000)
        resources["write_bytes"] = max(2 * resources["write_bytes"], 2_000)
    else:
        resources = {
            "footprint": {
                "read_only": [
                    {"contract_data": {"contract": contract,
                                       "key": "ledger_key_contract_instance",
                                       "durability": "persistent"}},
                    {"contract_code": {"hash": a.wasm_hash}},
                ],
                "read_write": [
                    {"contract_data": {"contract": contract, "key": json.loads(k),
                                       "durability": "persistent"}}
                    for k in a.key
                ],
            },
            "instructions": INSTRUCTIONS,
            "disk_read_bytes": 20_000,
            "write_bytes": 2_000,
        }
    tx["ext"] = {"v1": {"ext": "v0", "resources": resources, "resource_fee": str(RESOURCE_FEE)}}
    tx["fee"] = 100 + RESOURCE_FEE

    code, xdr, err = stellar("tx", "encode", stdin=json.dumps(env))
    if code:
        sys.exit(f"cannot encode envelope: {err}")
    code, signed, err = stellar("tx", "sign", "--network", a.network, "--sign-with-key", a.source,
                                stdin=xdr)
    if code:
        sys.exit(f"cannot sign: {err}")
    _, tx_hash, _ = stellar("tx", "hash", "--network", a.network, stdin=signed)
    code, out, err = stellar("tx", "send", "--network", a.network, stdin=signed)
    print(tx_hash)
    if code == 0:
        sys.exit("the call succeeded; it was expected to fail")
    if f'{{"contract":{a.expect}}}' not in err:
        sys.exit(f"the call failed, but not with contract error {a.expect}:\n{err}")


if __name__ == "__main__":
    main()

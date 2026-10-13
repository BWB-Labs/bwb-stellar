#!/usr/bin/env python3
"""Submit a contract call that is expected to fail, so the rejection is
recorded on the ledger with its error code.

The Stellar CLI simulates every call before sending it and stops when the
simulation fails, so a rejected call never gets a transaction hash. This
script skips simulation: it takes the unsimulated envelope from
`stellar contract invoke --build-only`, adds the authorization entry and
the resources by hand, then signs and sends it. The network executes it,
it fails with the contract's error, and the fee is still charged.

    stellar contract invoke --build-only ... | \
      scripts/lib/send_rejected.py --network testnet --source <identity> \
        --wasm-hash <hex> --expect <code> [--key '<ScVal json>' ...]

Each --key is a persistent storage key of the called contract that the
call may touch before it fails (whitelist entries, balances). The contract
instance and code are always in the footprint. Prints the transaction hash.
Exits 0 only when the transaction was included and failed with contract
error --expect; a call that succeeds or fails for another reason (a missing
footprint key, say) is an error.
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


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--network", required=True)
    p.add_argument("--source", required=True)
    p.add_argument("--wasm-hash", required=True)
    p.add_argument("--expect", required=True, type=int, help="expected contract error code")
    p.add_argument("--key", action="append", default=[])
    a = p.parse_args()

    code, out, err = stellar("tx", "decode", stdin=sys.stdin.read())
    if code:
        sys.exit(f"cannot decode envelope: {err}")
    env = json.loads(out)
    tx = env["tx"]["tx"]
    op = tx["operations"][0]["body"]["invoke_host_function"]
    call = op["host_function"]["invoke_contract"]
    contract = call["contract_address"]

    # The source account authorizes the root call, as recording simulation
    # would do for a require_auth on the source.
    op["auth"] = [{
        "credentials": "source_account",
        "root_invocation": {"function": {"contract_fn": call}, "sub_invocations": []},
    }]
    tx["ext"] = {"v1": {
        "ext": "v0",
        "resources": {
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
        },
        "resource_fee": str(RESOURCE_FEE),
    }}
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

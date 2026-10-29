import { type Asset, Operation, StrKey, xdr } from '@stellar/stellar-sdk/base'

import { accountKey, type StellarRpc } from './network.js'

export type SignerSet = {
  signers: Array<string>
  threshold: number
}

export type TreasuryCreation = SignerSet & {
  treasury: string
  sponsor: string
  asset: Asset
}

function thresholdsAt(threshold: number) {
  return {
    masterWeight: 0,
    lowThreshold: threshold,
    medThreshold: threshold,
    highThreshold: threshold,
  }
}

export function treasuryCreationOperations({
  treasury,
  sponsor,
  asset,
  signers,
  threshold,
}: TreasuryCreation): Array<xdr.Operation> {
  const lastSigner = signers.length - 1
  return [
    Operation.beginSponsoringFutureReserves({ sponsoredId: treasury, source: sponsor }),
    Operation.createAccount({ destination: treasury, startingBalance: '0', source: sponsor }),
    Operation.changeTrust({ asset, source: treasury }),
    ...signers.map((signer, index) =>
      Operation.setOptions({
        signer: { ed25519PublicKey: signer, weight: 1 },
        ...(index === lastSigner && thresholdsAt(threshold)),
        source: treasury,
      }),
    ),
    Operation.endSponsoringFutureReserves({ source: treasury }),
  ]
}

export type SignerChange = {
  treasury: string
  sponsor: string
  current: SignerSet
  target: SignerSet
}

export function signerChangeOperations({
  treasury,
  sponsor,
  current,
  target,
}: SignerChange): Array<xdr.Operation> {
  const added = target.signers.filter((signer) => !current.signers.includes(signer))
  const removed = current.signers.filter((signer) => !target.signers.includes(signer))
  const signerAt = (signer: string, weight: number) =>
    Operation.setOptions({ signer: { ed25519PublicKey: signer, weight }, source: treasury })
  return [
    ...(added.length > 0
      ? [
          Operation.beginSponsoringFutureReserves({ sponsoredId: treasury, source: sponsor }),
          ...added.map((signer) => signerAt(signer, 1)),
          Operation.endSponsoringFutureReserves({ source: treasury }),
        ]
      : []),
    ...removed.map((signer) => signerAt(signer, 0)),
    Operation.setOptions({ ...thresholdsAt(target.threshold), source: treasury }),
  ]
}

export type AccountEntry = ReturnType<xdr.AccountEntry['toXdrObject']>

export function signerSetOf(account: AccountEntry): SignerSet & { masterWeight: number } {
  const [masterWeight = 0, ...thresholds] = account.thresholds
  return {
    signers: account.signers.flatMap(({ key, weight }) =>
      key.type === 0 && weight > 0 ? [StrKey.encodeEd25519PublicKey(key.ed25519)] : [],
    ),
    threshold: Math.min(...thresholds),
    masterWeight,
  }
}

export async function readAccount(
  rpc: StellarRpc,
  address: string,
): Promise<AccountEntry | null> {
  const { entries } = await rpc((server) => server.getLedgerEntries(accountKey(address)))
  const [entry] = entries
  return entry?.val.type === 'account' ? entry.val.account.toXdrObject() : null
}

export async function readSignerSet(
  rpc: StellarRpc,
  address: string,
): Promise<SignerSet | null> {
  const account = await readAccount(rpc, address)
  if (account === null) return null
  const { masterWeight, signers, threshold } = signerSetOf(account)
  return { signers: masterWeight > 0 ? [address, ...signers] : signers, threshold }
}

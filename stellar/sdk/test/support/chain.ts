import {
  Keypair,
  Networks,
  SorobanDataBuilder,
  TransactionBuilder,
  xdr,
  type FeeBumpTransaction,
} from '@stellar/stellar-sdk/base'
import { vi } from 'vitest'

export const RPC_URL = 'https://rpc.example'

type JsonRpcRequest = { host: string; method: string; params: unknown }

const RPC_ERROR = Symbol('rpcError')

export function rpcError(code: number, message: string) {
  return { [RPC_ERROR]: { code, message } }
}

export function stubJsonRpc(answer: (request: JsonRpcRequest) => unknown): Array<JsonRpcRequest> {
  const requests: Array<JsonRpcRequest> = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit) => {
      const { id, method, params } = JSON.parse(String(init.body))
      const request = { host: new URL(url).host, method, params }
      requests.push(request)
      const answered = answer(request)
      const body =
        typeof answered === 'object' && answered !== null && RPC_ERROR in answered
          ? { error: answered[RPC_ERROR] }
          : { result: answered }
      return new Response(JSON.stringify({ jsonrpc: '2.0', id, ...body }), {
        headers: { 'Content-Type': 'application/json' },
      })
    }),
  )
  return requests
}

function firstRequestedKey(params: unknown): string {
  if (
    typeof params === 'object' &&
    params !== null &&
    'keys' in params &&
    Array.isArray(params.keys) &&
    typeof params.keys[0] === 'string'
  ) {
    return params.keys[0]
  }
  throw new Error('No ledger key requested')
}

export type Thresholds = [number, number, number, number]

export function accountEntry(
  account: string,
  sequence: bigint,
  { thresholds = [1, 0, 0, 0], signers = [] }: { thresholds?: Thresholds; signers?: Array<[string, number]> } = {},
): string {
  return xdr.LedgerEntryData.account(
    new xdr.AccountEntry({
      accountId: Keypair.fromPublicKey(account).xdrPublicKey(),
      balance: 0n,
      seqNum: sequence,
      numSubEntries: signers.length,
      inflationDest: null,
      flags: 0,
      homeDomain: '',
      thresholds: Buffer.from(thresholds),
      signers: signers.map(
        ([signer, weight]) =>
          new xdr.Signer({
            key: xdr.SignerKey.signerKeyTypeEd25519(Keypair.fromPublicKey(signer).rawPublicKey()),
            weight,
          }),
      ),
      ext: xdr.AccountEntryExt.v0(),
    }),
  ).toXdr('base64')
}

export function transactionResult(code: xdr.TransactionResultResult): string {
  return new xdr.TransactionResult({
    feeCharged: 100n,
    result: code,
    ext: xdr.TransactionResultExt.v0(),
  }).toXdr('base64')
}

export function innerFailed(code: xdr.InnerTransactionResultResult): string {
  return transactionResult(
    xdr.TransactionResultResult.txFeeBumpInnerFailed(
      new xdr.InnerTransactionResultPair({
        transactionHash: Buffer.alloc(32),
        result: new xdr.InnerTransactionResult({
          feeCharged: 0n,
          result: code,
          ext: xdr.InnerTransactionResultExt.v0(),
        }),
      }),
    ),
  )
}

export function simulationSucceeded(resourceFee = '500'): Record<string, unknown> {
  return {
    minResourceFee: resourceFee,
    transactionData: new SorobanDataBuilder().setResourceFee(resourceFee).build().toXDR('base64'),
    results: [{ xdr: xdr.ScVal.scvVoid().toXDR('base64'), auth: [] }],
    events: [],
  }
}

export type ChainAnswers = {
  accounts?: Array<string | undefined>
  sent?: Array<Record<string, unknown>>
  polled?: Array<Record<string, unknown>>
  simulated?: Record<string, unknown>
  closeTime?: number
  passphrase?: string
}

function next<T>(queue: Array<T>): T | undefined {
  return queue.length > 1 ? queue.shift() : queue[0]
}

export function stubChain({
  accounts = [],
  sent = [{ status: 'PENDING' }],
  polled = [{ status: 'SUCCESS', ledger: 9, createdAt: '1793000000' }],
  simulated = simulationSucceeded(),
  closeTime = 0,
  passphrase = Networks.TESTNET,
}: ChainAnswers) {
  const entries = [...accounts]
  const sends = [...sent]
  const polls = [...polled]
  return stubJsonRpc(({ method, params }) => {
    if (method === 'getNetwork') return { passphrase, protocolVersion: 23 }
    if (method === 'getLedgerEntries') {
      const entry = next(entries)
      return {
        entries: entry === undefined ? [] : [{ key: firstRequestedKey(params), xdr: entry, lastModifiedLedgerSeq: 1 }],
        latestLedger: 7,
      }
    }
    if (method === 'simulateTransaction') return { latestLedger: 7, ...simulated }
    if (method === 'sendTransaction') {
      const answer = next(sends)
      if (answer !== undefined && 'error' in answer) return answer.error
      return { hash: 'ab'.repeat(32), latestLedger: 7, latestLedgerCloseTime: '0', ...answer }
    }
    return {
      latestLedger: 8,
      latestLedgerCloseTime: String(closeTime),
      oldestLedger: 1,
      oldestLedgerCloseTime: '0',
      ...next(polls),
    }
  })
}

export function sentEnvelopes(requests: Array<JsonRpcRequest>): Array<FeeBumpTransaction> {
  return requests
    .filter(({ method }) => method === 'sendTransaction')
    .map(({ params }) => {
      if (typeof params === 'object' && params !== null && 'transaction' in params && typeof params.transaction === 'string') {
        const sent = TransactionBuilder.fromXDR(params.transaction, Networks.TESTNET)
        if ('innerTransaction' in sent) return sent
        throw new Error('Transaction sent without a fee-bump')
      }
      throw new Error('No transaction sent')
    })
}

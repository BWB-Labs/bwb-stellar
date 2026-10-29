import {
  Account,
  BASE_FEE,
  type FeeBumpTransaction,
  Transaction,
  TransactionBuilder,
  xdr,
} from '@stellar/stellar-sdk/base'
import type { Api } from '@stellar/stellar-sdk/rpc'

import { accountKey, type StellarRpc } from './network.js'
import { refuse, succeed, type Result, type ResultError } from './result.js'
import { fromHex, toHex, type AddressSignature, type HashSigner } from './signing.js'
import { invokesHostFunction, prepareSorobanTransaction, SorobanSimulationError } from './soroban.js'

export const RELAY_FAILURE = {
  PREPARATION_FAILED: 'preparationFailed',
  SIMULATION_REVERTED: 'simulationReverted',
  SIGNING_FAILED: 'signingFailed',
  SUBMISSION_FAILED: 'submissionFailed',
  RELAY_ACCOUNT_MISSING: 'relayAccountMissing',
  TIME_BOUND_PASSED: 'timeBoundPassed',
  TRANSACTION_FAILED: 'transactionFailed',
} as const

export type RelayFailureCode = (typeof RELAY_FAILURE)[keyof typeof RELAY_FAILURE]

export type RelayFailure = {
  code: RelayFailureCode
  providerMessage: string
  transactionHash?: string
}

export type Submission = {
  transactionHash: string
  feeBumpHash: string
  ledger: number
  settledAt: number
}

export type SubmitResult = Result<Submission, RelayFailure>

export type PlatformExecution = {
  channel: HashSigner
  operations: Array<xdr.Operation>
  signers: Array<HashSigner>
  timeBound: number
}

export type SignerExecution = {
  source: string
  operations: Array<xdr.Operation>
  timeBound: number
}

export type SigningRequest = {
  envelope: string
  hash: string
}

export type SignedEnvelope = {
  envelope: string
  signatures: Array<AddressSignature>
  platformSigners: Array<HashSigner>
}

export type Relay = {
  submitAsPlatform(execution: PlatformExecution): Promise<SubmitResult>
  prepareForSigners(execution: SignerExecution): Promise<Result<SigningRequest, RelayFailure>>
  submitSigned(signed: SignedEnvelope): Promise<SubmitResult>
}

export type RelaySettings = {
  rpc: StellarRpc
  sponsor: HashSigner
  sequenceAttempts?: number
  pollIntervalMs?: number
  signedPollMs?: number
}

const CONTEXT = 'Stellar relay'

type Hashes = Pick<Submission, 'transactionHash' | 'feeBumpHash'>

function failed(
  code: RelayFailureCode,
  providerMessage: string,
  transactionHash?: string,
): ResultError<RelayFailure> {
  return refuse(
    { code, providerMessage, ...(transactionHash !== undefined && { transactionHash }) },
    `${CONTEXT}: ${providerMessage}`,
  )
}

function resultCode(resultXdr: string | undefined): string {
  if (resultXdr === undefined) return 'no result'
  const { result } = xdr.TransactionResult.fromXdr(resultXdr, 'base64')
  return result.type === 'txFeeBumpInnerFailed'
    ? result.innerResultPair.result.result.type
    : result.type
}

function sentFailure(
  { status, errorResultXdr }: Api.RawSendTransactionResponse,
  { transactionHash }: Hashes,
): SubmitResult | undefined {
  if (status === 'TRY_AGAIN_LATER') {
    return failed(RELAY_FAILURE.SUBMISSION_FAILED, 'Network asked to try again later')
  }
  if (status === 'ERROR') {
    const code = resultCode(errorResultXdr)
    return failed(
      code === 'txTooLate' ? RELAY_FAILURE.TIME_BOUND_PASSED : RELAY_FAILURE.TRANSACTION_FAILED,
      code,
      transactionHash,
    )
  }
  return undefined
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function hashesOf(bumped: FeeBumpTransaction): Hashes {
  return {
    transactionHash: toHex(bumped.innerTransaction.hash()),
    feeBumpHash: toHex(bumped.hash()),
  }
}

function toBase64(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('base64')
}

async function signWith(
  transaction: Transaction | FeeBumpTransaction,
  signers: Array<HashSigner>,
): Promise<void> {
  const hash = transaction.hash()
  const signatures = await Promise.all(signers.map((signer) => signer.sign(hash)))
  signers.forEach(({ address }, index) => {
    transaction.addSignature(address, toBase64(signatures[index]))
  })
}

export function createRelay({
  rpc,
  sponsor,
  sequenceAttempts = 3,
  pollIntervalMs = 1_000,
  signedPollMs = 60_000,
}: RelaySettings): Relay {
  async function buildFrom(
    source: string,
    timeBound: number,
    operations: Array<xdr.Operation>,
  ): Promise<Transaction | null> {
    return rpc(async (server, passphrase) => {
      const { entries } = await server.getLedgerEntries(accountKey(source))
      const [entry] = entries
      if (entry?.val.type !== 'account') return null
      const transaction = operations
        .reduce(
          (builder, operation) => builder.addOperation(operation),
          new TransactionBuilder(new Account(source, entry.val.account.seqNum.toString()), {
            fee: BASE_FEE,
            networkPassphrase: passphrase,
          }).setTimebounds(0, timeBound),
        )
        .build()
      if (!operations.some(invokesHostFunction)) return transaction
      return prepareSorobanTransaction(server, transaction)
    })
  }

  async function bumped(inner: Transaction): Promise<FeeBumpTransaction> {
    // shortcut: no re-bump at a higher fee for a stuck transaction, when mainnet traffic meets surge pricing
    const bump = TransactionBuilder.buildFeeBumpTransaction(
      sponsor.address,
      BASE_FEE,
      inner,
      inner.networkPassphrase,
    )
    await signWith(bump, [sponsor])
    return bump
  }

  function awaitFinal(hashes: Hashes, timeBound: number, pollUntil = Number.POSITIVE_INFINITY): Promise<SubmitResult> {
    const { transactionHash } = hashes
    return rpc(async (server) => {
      for (;;) {
        const found = await server._getTransaction(transactionHash)
        if (found.status === 'SUCCESS') {
          return succeed(
            { ...hashes, ledger: Number(found.ledger), settledAt: Number(found.createdAt) * 1000 },
            'Stellar transaction applied',
          )
        }
        if (found.status === 'FAILED') {
          return failed(RELAY_FAILURE.TRANSACTION_FAILED, resultCode(found.resultXdr), transactionHash)
        }
        if (Number(found.latestLedgerCloseTime) > timeBound) {
          return failed(
            RELAY_FAILURE.TIME_BOUND_PASSED,
            `Transaction ${transactionHash} did not land by ${timeBound}`,
            transactionHash,
          )
        }
        if (Date.now() >= pollUntil) {
          return failed(
            RELAY_FAILURE.SUBMISSION_FAILED,
            `Transaction ${transactionHash} has not landed yet and may still land`,
            transactionHash,
          )
        }
        await sleep(pollIntervalMs)
      }
    })
  }

  return {
    async submitAsPlatform({ channel, operations, signers, timeBound }) {
      for (let attempt = 1; ; attempt += 1) {
        let transaction: Transaction | null
        try {
          transaction = await buildFrom(channel.address, timeBound, operations)
        } catch (error) {
          return failed(
            error instanceof SorobanSimulationError
              ? RELAY_FAILURE.SIMULATION_REVERTED
              : RELAY_FAILURE.PREPARATION_FAILED,
            String(error),
          )
        }
        if (transaction === null) {
          return failed(
            RELAY_FAILURE.RELAY_ACCOUNT_MISSING,
            `Channel account ${channel.address} holds no account on chain`,
          )
        }

        let bump: FeeBumpTransaction
        try {
          await signWith(transaction, [channel, ...signers])
          bump = await bumped(transaction)
        } catch (error) {
          return failed(RELAY_FAILURE.SIGNING_FAILED, String(error))
        }

        const hashes = hashesOf(bump)
        try {
          const sent = await rpc((server) => server._sendTransaction(bump))
          if (
            sent.status === 'ERROR' &&
            resultCode(sent.errorResultXdr) === 'txBadSeq' &&
            attempt < sequenceAttempts
          ) {
            continue
          }
          return sentFailure(sent, hashes) ?? (await awaitFinal(hashes, timeBound))
        } catch (error) {
          return failed(
            RELAY_FAILURE.SUBMISSION_FAILED,
            `Transaction ${hashes.transactionHash} may still land: ${String(error)}`,
            hashes.transactionHash,
          )
        }
      }
    },

    async prepareForSigners({ source, operations, timeBound }) {
      let transaction: Transaction | null
      try {
        transaction = await buildFrom(source, timeBound, operations)
      } catch (error) {
        return failed(RELAY_FAILURE.PREPARATION_FAILED, String(error))
      }
      if (transaction === null) {
        return failed(
          RELAY_FAILURE.RELAY_ACCOUNT_MISSING,
          `Source account ${source} holds no account on chain`,
        )
      }
      return succeed(
        { envelope: transaction.toXDR(), hash: toHex(transaction.hash()) },
        'Stellar transaction prepared for its signers',
      )
    },

    async submitSigned({ envelope, signatures, platformSigners }) {
      const passphrase = await rpc(async (_server, passphrase) => passphrase)
      const transaction = new Transaction(envelope, passphrase)
      const sources = new Set([
        transaction.source,
        ...transaction.operations.flatMap(({ source }) => (source === undefined ? [] : [source])),
      ])
      let bump: FeeBumpTransaction
      try {
        signatures.forEach(({ address, signature }) =>
          transaction.addSignature(address, toBase64(fromHex(signature))),
        )
        await signWith(
          transaction,
          platformSigners.filter(({ address }) => sources.has(address)),
        )
        bump = await bumped(transaction)
      } catch (error) {
        return failed(RELAY_FAILURE.SIGNING_FAILED, String(error))
      }
      const hashes = hashesOf(bump)
      const timeBound = Number(transaction.timeBounds?.maxTime)
      try {
        return await rpc(async (server) => {
          const found = await server._getTransaction(hashes.transactionHash)
          if (found.status !== 'NOT_FOUND') return awaitFinal(hashes, timeBound)
          return (
            sentFailure(await server._sendTransaction(bump), hashes) ??
            awaitFinal(hashes, timeBound, Date.now() + signedPollMs)
          )
        })
      } catch (error) {
        return failed(
          RELAY_FAILURE.SUBMISSION_FAILED,
          `Transaction ${hashes.transactionHash} may still land: ${String(error)}`,
          hashes.transactionHash,
        )
      }
    },
  }
}

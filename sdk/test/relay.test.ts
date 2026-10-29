import { BASE_FEE, Keypair, Networks, Operation, Transaction, xdr } from '@stellar/stellar-sdk/base'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { connect, NETWORK } from '../src/network.js'
import { createRelay } from '../src/relay.js'
import { keypairSigner, toHex } from '../src/signing.js'
import {
  accountEntry,
  innerFailed,
  rpcError,
  RPC_URL,
  sentEnvelopes,
  simulationSucceeded,
  stubChain,
} from './support/chain.js'

const CHANNEL = Keypair.random()
const SPONSOR = Keypair.random()
const CREATION_KEY = Keypair.random()
const OFFICER = Keypair.random()
const TREASURY = Keypair.random().publicKey()
const SALE = 'CCL2OOMX4NYI4Y6RDYHFRJ6VKNKLVM7POOZIMKJUSJRUTETDKAIPK5IT'

const TIME_BOUND = 1_793_000_060
const SEQUENCE = 4_100_000_000_007n
const CHANNEL_ACCOUNT = accountEntry(CHANNEL.publicKey(), SEQUENCE)

function relay() {
  return createRelay({
    rpc: connect({ network: NETWORK.TESTNET, rpcUrls: [RPC_URL] }),
    sponsor: keypairSigner(SPONSOR),
    pollIntervalMs: 0,
  })
}

function submitTreasuryCreation() {
  return relay().submitAsPlatform({
    channel: keypairSigner(CHANNEL),
    operations: [
      Operation.beginSponsoringFutureReserves({ sponsoredId: CREATION_KEY.publicKey(), source: SPONSOR.publicKey() }),
      Operation.endSponsoringFutureReserves({ source: CREATION_KEY.publicKey() }),
    ],
    signers: [keypairSigner(SPONSOR), keypairSigner(CREATION_KEY)],
    timeBound: TIME_BOUND,
  })
}

async function preparedSignerChange() {
  const prepared = await relay().prepareForSigners({
    source: CHANNEL.publicKey(),
    operations: [Operation.setOptions({ masterWeight: 0, source: TREASURY })],
    timeBound: TIME_BOUND,
  })
  if (!prepared.success) throw new Error(prepared.message)
  return prepared.data
}

function signedByOfficer(hash: string) {
  return {
    address: OFFICER.publicKey(),
    signature: `0x${toHex(OFFICER.sign(Buffer.from(hash, 'hex')))}`,
  }
}

const PLATFORM = [keypairSigner(CHANNEL), keypairSigner(SPONSOR)]

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('submitAsPlatform', () => {
  it('submits from the channel signed by it and the named keys, fee-bumped by the sponsor, and returns both hashes', async () => {
    const requests = stubChain({ accounts: [CHANNEL_ACCOUNT] })

    const submitted = await submitTreasuryCreation()

    const [bump] = sentEnvelopes(requests)
    const envelope = bump!.innerTransaction
    expect(submitted).toEqual({
      success: true,
      data: {
        transactionHash: toHex(envelope.hash()),
        feeBumpHash: toHex(bump!.hash()),
        ledger: 9,
        settledAt: 1_793_000_000_000,
      },
      message: 'Stellar transaction applied',
    })
    expect(envelope.source).toBe(CHANNEL.publicKey())
    expect(envelope.sequence).toBe(String(SEQUENCE + 1n))
    expect(envelope.timeBounds?.maxTime).toBe(String(TIME_BOUND))
    expect(
      [CHANNEL, SPONSOR, CREATION_KEY].map((keypair) =>
        envelope.signatures.some(({ signature }) => keypair.verify(envelope.hash(), signature)),
      ),
    ).toEqual([true, true, true])
    expect(bump!.feeSource).toBe(SPONSOR.publicKey())
    expect(Number(bump!.fee)).toBe(Number(envelope.fee) + Number(BASE_FEE))
    expect(bump!.signatures.map(({ signature }) => SPONSOR.verify(bump!.hash(), signature))).toEqual([true])
  })

  it('simulates a contract call and submits it with the resources the network asked for', async () => {
    const requests = stubChain({ accounts: [CHANNEL_ACCOUNT], simulated: simulationSucceeded('1234') })

    const submitted = await relay().submitAsPlatform({
      channel: keypairSigner(CHANNEL),
      operations: [Operation.invokeContractFunction({ contract: SALE, function: 'activate', args: [], source: CREATION_KEY.publicKey() })],
      signers: [keypairSigner(CREATION_KEY)],
      timeBound: TIME_BOUND,
    })

    expect(submitted.success).toBe(true)
    const [bump] = sentEnvelopes(requests)
    expect(Number(bump!.innerTransaction.fee)).toBe(Number(BASE_FEE) + 1234)
  })

  it('reads the sequence again and resubmits when the chain refuses a stale one', async () => {
    const requests = stubChain({
      accounts: [CHANNEL_ACCOUNT, accountEntry(CHANNEL.publicKey(), SEQUENCE + 1n)],
      sent: [
        { status: 'ERROR', errorResultXdr: innerFailed(xdr.InnerTransactionResultResult.txBadSeq()) },
        { status: 'PENDING' },
      ],
    })

    const submitted = await submitTreasuryCreation()

    expect(submitted.success).toBe(true)
    expect(sentEnvelopes(requests).map(({ innerTransaction }) => innerTransaction.sequence)).toEqual([
      String(SEQUENCE + 1n),
      String(SEQUENCE + 2n),
    ])
  })

  it('stops resubmitting after three stale sequences and names the refusal', async () => {
    const requests = stubChain({
      accounts: [CHANNEL_ACCOUNT],
      sent: [{ status: 'ERROR', errorResultXdr: innerFailed(xdr.InnerTransactionResultResult.txBadSeq()) }],
    })

    const submitted = await submitTreasuryCreation()

    expect(submitted).toMatchObject({ success: false, error: { code: 'transactionFailed', providerMessage: 'txBadSeq' } })
    expect(sentEnvelopes(requests)).toHaveLength(3)
  })

  it('refuses with time bound passed when the network finds the transaction too late', async () => {
    stubChain({
      accounts: [CHANNEL_ACCOUNT],
      sent: [{ status: 'ERROR', errorResultXdr: innerFailed(xdr.InnerTransactionResultResult.txTooLate()) }],
    })

    expect(await submitTreasuryCreation()).toMatchObject({ success: false, error: { code: 'timeBoundPassed' } })
  })

  it('refuses with time bound passed once the chain has closed past it without the transaction', async () => {
    stubChain({ accounts: [CHANNEL_ACCOUNT], polled: [{ status: 'NOT_FOUND' }], closeTime: TIME_BOUND + 1 })

    expect(await submitTreasuryCreation()).toMatchObject({ success: false, error: { code: 'timeBoundPassed' } })
  })

  it('says the transaction may still land, naming it, when the network stops answering after submission', async () => {
    stubChain({ accounts: [CHANNEL_ACCOUNT], sent: [{ error: rpcError(-32603, 'internal error') }] })

    const submitted = await submitTreasuryCreation()

    expect(submitted).toMatchObject({
      success: false,
      error: { code: 'submissionFailed', transactionHash: expect.stringMatching(/^[0-9a-f]{64}$/) },
    })
  })

  it('refuses with relay account missing, sending nothing, when the channel has no account on chain', async () => {
    const requests = stubChain({})

    expect(await submitTreasuryCreation()).toMatchObject({ success: false, error: { code: 'relayAccountMissing' } })
    expect(sentEnvelopes(requests)).toEqual([])
  })

  it('refuses with signing failed, sending nothing, when a platform key does not sign', async () => {
    const requests = stubChain({ accounts: [CHANNEL_ACCOUNT] })

    const submitted = await relay().submitAsPlatform({
      channel: keypairSigner(CHANNEL),
      operations: [Operation.setOptions({ masterWeight: 0, source: TREASURY })],
      signers: [{ address: TREASURY, sign: () => Promise.reject(new Error('custody unavailable')) }],
      timeBound: TIME_BOUND,
    })

    expect(submitted).toMatchObject({ success: false, error: { code: 'signingFailed' } })
    expect(sentEnvelopes(requests)).toEqual([])
  })
})

describe('prepareForSigners', () => {
  it('returns the unsigned envelope from the given source with the hash its signers sign', async () => {
    stubChain({ accounts: [CHANNEL_ACCOUNT] })

    const { envelope, hash } = await preparedSignerChange()

    const transaction = new Transaction(envelope, Networks.TESTNET)
    expect(transaction.signatures).toEqual([])
    expect(transaction.source).toBe(CHANNEL.publicKey())
    expect(transaction.timeBounds?.maxTime).toBe(String(TIME_BOUND))
    expect(hash).toBe(toHex(transaction.hash()))
  })
})

describe('submitSigned', () => {
  it('adds the officers’ signatures and the platform’s, and sends the envelope fee-bumped by the sponsor', async () => {
    const requests = stubChain({ accounts: [CHANNEL_ACCOUNT], polled: [{ status: 'NOT_FOUND' }, { status: 'SUCCESS', ledger: 9, createdAt: '1793000000' }] })
    const { envelope, hash } = await preparedSignerChange()

    const submitted = await relay().submitSigned({ envelope, signatures: [signedByOfficer(hash)], platformSigners: PLATFORM })

    expect(submitted.success).toBe(true)
    const [bump] = sentEnvelopes(requests)
    const sent = bump!.innerTransaction
    expect([OFFICER, CHANNEL].map((keypair) => sent.signatures.some(({ signature }) => keypair.verify(sent.hash(), signature)))).toEqual([true, true])
    expect(bump!.feeSource).toBe(SPONSOR.publicKey())
  })

  it('sends nothing and reports the outcome when the envelope already landed', async () => {
    const requests = stubChain({ accounts: [CHANNEL_ACCOUNT] })
    const { envelope, hash } = await preparedSignerChange()

    const submitted = await relay().submitSigned({ envelope, signatures: [signedByOfficer(hash)], platformSigners: PLATFORM })

    expect(submitted.success).toBe(true)
    expect(sentEnvelopes(requests)).toEqual([])
  })

  it('refuses with signing failed, sending nothing, when an officer’s signature is not of the envelope', async () => {
    const requests = stubChain({ accounts: [CHANNEL_ACCOUNT], polled: [{ status: 'NOT_FOUND' }] })
    const { envelope } = await preparedSignerChange()

    const submitted = await relay().submitSigned({ envelope, signatures: [signedByOfficer('ab'.repeat(32))], platformSigners: PLATFORM })

    expect(submitted).toMatchObject({ success: false, error: { code: 'signingFailed' } })
    expect(sentEnvelopes(requests)).toEqual([])
  })

  it('refuses naming the failure when the network rejects the envelope', async () => {
    stubChain({
      accounts: [CHANNEL_ACCOUNT],
      polled: [{ status: 'NOT_FOUND' }],
      sent: [{ status: 'ERROR', errorResultXdr: innerFailed(xdr.InnerTransactionResultResult.txBadAuth()) }],
    })
    const { envelope, hash } = await preparedSignerChange()

    const submitted = await relay().submitSigned({ envelope, signatures: [signedByOfficer(hash)], platformSigners: PLATFORM })

    expect(submitted).toMatchObject({ success: false, error: { code: 'transactionFailed', providerMessage: 'txBadAuth' } })
  })

  it('says the envelope may still land when it has not landed within a minute', async () => {
    stubChain({ accounts: [CHANNEL_ACCOUNT], polled: [{ status: 'NOT_FOUND' }] })
    const { envelope, hash } = await preparedSignerChange()
    vi.useFakeTimers({ toFake: ['setTimeout', 'Date'] })

    const submitting = relay().submitSigned({ envelope, signatures: [signedByOfficer(hash)], platformSigners: PLATFORM })
    await vi.advanceTimersByTimeAsync(61_000)

    expect(await submitting).toMatchObject({ success: false, error: { code: 'submissionFailed' } })
  })
})

import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import { Asset, Keypair, Networks, Operation, xdr } from '@stellar/stellar-sdk/base'
import type { Server } from '@stellar/stellar-sdk/rpc'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'

import {
  createRelay,
  deploymentDigest,
  fromHex,
  isSignatureOf,
  keypairSigner,
  NETWORK,
  networkIdOf,
  offeringTerms,
  privySigner,
  toHex,
  treasuryCreationOperations,
  walletOpeningOperations,
  type StellarRpc,
} from '../src/index.js'
import { accountEntry } from './stellar/support/chain.js'

const SOURCE = join(import.meta.dirname, '../src')
const KEY = Keypair.random()
const CHANNEL = Keypair.random()
const SPONSOR = Keypair.random()
const HASH = 'cd'.repeat(32)
const SIGNATURE = `0x${toHex(KEY.sign(fromHex(HASH)))}`
const CHANNEL_ACCOUNT = xdr.LedgerEntryData.fromXDR(accountEntry(CHANNEL.publicKey(), 7n), 'base64')

function terms() {
  return offeringTerms(
    {
      offeringId: 'offering-1',
      treasury: KEY.publicKey(),
      tokenName: 'Aurora',
      tokenSymbol: 'AURORA',
      offeringURI: 'https://bwb.example/offerings/offering-1',
      totalTokens: '1000',
      pricePerQuota: '10000000',
      coolingOffSeconds: '432000',
    },
    {
      network: NETWORK.TESTNET,
      platform: KEY.publicKey(),
      platformAdmin: KEY.publicKey(),
      paymentAsset: 'CCW4PRV5CXNVX3NBPXL5GJ4AEGRNCGQAE3TYBXXK3E6E3HPR3DTXNJGY',
      allowlist: 'CAVI4RROECISQXR4BR3O2BYH6NJFE7WI6NU6UTTHZKX3VCR6J6PWEDRQ',
      tokenWasmHash: 'a209117501d643638a1b434b38a5d8bb203394bdb98daa8678e82b6e231cadd0',
      saleWasmHash: '49edfc275630882ce01e9a190a84f0f833ad3e48183ddd17ea2487463b0af6bd',
    },
  )
}

function sourceFiles(directory: string): Array<string> {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? sourceFiles(join(directory, entry.name)) : [join(directory, entry.name)],
  )
}

describe('a runtime with no global Buffer', () => {
  beforeAll(() => {
    vi.stubGlobal('Buffer', undefined)
  })

  afterAll(() => {
    vi.unstubAllGlobals()
  })

  it('leaves no module naming Buffer', () => {
    const naming = sourceFiles(SOURCE).filter((file) => /\bBuffer\b/.test(readFileSync(file, 'utf8')))

    expect(naming).toEqual([])
  })

  it('converts hex both ways and derives the network id', () => {
    expect(toHex(fromHex(`0x${HASH}`))).toBe(HASH)
    expect(toHex(networkIdOf(NETWORK.TESTNET))).toBe(
      'cee0302d59844d32bdca915c8203dd44b33fbb7edc19051ea37abedf28ecd472',
    )
  })

  it('accepts a real signature and refuses another hash', () => {
    expect(isSignatureOf(KEY.publicKey(), HASH, SIGNATURE)).toBe(true)
    expect(isSignatureOf(KEY.publicKey(), 'ef'.repeat(32), SIGNATURE)).toBe(false)
  })

  it('signs through a keypair and through Privy', async () => {
    const bytes = fromHex(HASH)
    const rawSign = async () => ({ signature: SIGNATURE })

    expect(toHex(await keypairSigner(KEY).sign(bytes))).toBe(SIGNATURE.slice(2))
    expect(
      toHex(await privySigner({ rawSign }, { walletId: 'wallet-1', address: KEY.publicKey() }).sign(bytes)),
    ).toBe(SIGNATURE.slice(2))
  })

  it('derives an offering and its deployment digest', () => {
    const offering = terms()

    expect(offering.sale).toMatch(/^C[A-Z2-7]{55}$/)
    expect(deploymentDigest(NETWORK.TESTNET, offering)).toMatch(/^[0-9a-f]{64}$/)
  })

  it('builds wallet and treasury operations', () => {
    const asset = new Asset('USDC', KEY.publicKey())

    expect(
      walletOpeningOperations({
        address: KEY.publicKey(),
        sponsor: SPONSOR.publicKey(),
        asset,
        opensAccount: true,
      }).length,
    ).toBeGreaterThan(0)
    expect(
      treasuryCreationOperations({
        treasury: KEY.publicKey(),
        sponsor: SPONSOR.publicKey(),
        asset,
        signers: [CHANNEL.publicKey(), SPONSOR.publicKey()],
        threshold: 2,
      }).length,
    ).toBeGreaterThan(0)
  })

  it('signs and submits an envelope through the relay', async () => {
    const server = {
      getLedgerEntries: async () => ({ entries: [{ val: CHANNEL_ACCOUNT }] }),
      _sendTransaction: async () => ({ status: 'PENDING' }),
      _getTransaction: async () => ({ status: 'SUCCESS', ledger: 9, createdAt: '1793000000' }),
    } as unknown as Server
    const rpc: StellarRpc = (call) => call(server, Networks.TESTNET)

    const submitted = await createRelay({
      rpc,
      sponsor: keypairSigner(SPONSOR),
      pollIntervalMs: 0,
    }).submitAsPlatform({
      channel: keypairSigner(CHANNEL),
      operations: [Operation.bumpSequence({ bumpTo: '9' })],
      signers: [],
      timeBound: 1_793_000_060,
    })

    expect(submitted).toMatchObject({ success: true })
  })
})

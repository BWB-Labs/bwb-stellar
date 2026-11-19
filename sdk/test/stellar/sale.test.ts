import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import {
  Address,
  buildAuthorizationEntryPreimage,
  hash,
  Keypair,
  nativeToScVal,
  Networks,
  scValToNative,
  SorobanDataBuilder,
  Transaction,
  xdr,
} from '@stellar/stellar-sdk/base'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CHAIN_ID, createBwbSdk, SALE_REFUSAL, SALE_STATE, type PreparedPurchase, type Sale, type StellarChainId } from '../../src/index.js'
import { connect, NETWORK, type Network } from '../../src/stellar/network.js'
import { createRelay } from '../../src/stellar/relay.js'
import { fromHex, keypairSigner, toHex } from '../../src/stellar/signing.js'
import { scv } from '../../src/stellar/soroban.js'
import { accountEntry, RPC_URL, stubChain, stubJsonRpc } from './support/chain.js'

const RECEIPT_BUYER = 'GBW4N3ZYILVVU3R7B4ONAEMZS7PN6FTGD4OIY2ROPTK4CKHAHPU6JEJM'
const RECEIPT = readFileSync(join(import.meta.dirname, 'fixtures/purchase-meta.xdr'), 'utf8').trim()

const SALE = 'CDN3QJI4KSAULT4I7TQ5PO5SBPDPW3VIUDOAGUFVHUHKLLEZZUYHV4AC'
const USDC = 'CCTIP7NU7SLPRO3CL5FXW7BSPZGSTQUZFE44SNVSV6ZBRDPC5Z2JILBM'
const BUYER = Keypair.random()
const AMOUNT = 300_000_000n
const LATEST_LEDGER = 5_000
const EXPIRATION_LEDGER = 5_360

function contractCall(contract: string, method: string, args: Array<xdr.ScVal>) {
  return xdr.SorobanAuthorizedFunction.sorobanAuthorizedFunctionTypeContractFn(
    new xdr.InvokeContractArgs({
      contractAddress: Address.fromString(contract).toScAddress(),
      functionName: method,
      args,
    }),
  )
}

function unsignedEntry(signer: string, rootInvocation: xdr.SorobanAuthorizedInvocation) {
  return new xdr.SorobanAuthorizationEntry({
    credentials: xdr.SorobanCredentials.sorobanCredentialsAddressV2(
      new xdr.SorobanAddressCredentials({
        address: Address.fromString(signer).toScAddress(),
        nonce: 7n,
        signatureExpirationLedger: 0,
        signature: xdr.ScVal.scvVoid(),
      }),
    ),
    rootInvocation,
  })
}

function purchaseEntry({
  recipient = SALE,
  buyer = BUYER.publicKey(),
  signer = BUYER.publicKey(),
}: { recipient?: string; buyer?: string; signer?: string } = {}) {
  return unsignedEntry(
    signer,
    new xdr.SorobanAuthorizedInvocation({
      function: contractCall(SALE, 'buy', [scv.address(buyer), scv.i128(AMOUNT)]),
      subInvocations: [
        new xdr.SorobanAuthorizedInvocation({
          function: contractCall(USDC, 'transfer', [scv.address(buyer), scv.address(recipient), scv.i128(AMOUNT)]),
          subInvocations: [],
        }),
      ],
    }),
  )
}

function simulated(auth: Array<xdr.SorobanAuthorizationEntry>, latestLedger = LATEST_LEDGER, resourceFee = '500') {
  return {
    latestLedger,
    minResourceFee: resourceFee,
    transactionData: new SorobanDataBuilder().setResourceFee(resourceFee).build().toXDR('base64'),
    results: [{ xdr: xdr.ScVal.scvVoid().toXDR('base64'), auth: auth.map((entry) => entry.toXDR('base64')) }],
    events: [],
  }
}

function stubSimulation(answer: Record<string, unknown>, passphrase: string = Networks.TESTNET) {
  return stubJsonRpc(({ method }) => (method === 'getNetwork' ? { passphrase, protocolVersion: 23 } : answer))
}

function rpc(network: Network = NETWORK.TESTNET) {
  return connect({ network, rpcUrls: [RPC_URL] })
}

function theSale(address = SALE, chainId: StellarChainId = CHAIN_ID.STELLAR_TESTNET, network?: Network): Sale {
  const client = createBwbSdk({ chains: { [chainId]: { rpc: rpc(network) } } }).select(chainId)
  if (!client.success) throw new Error(client.message)
  return client.data.sale({ address })
}

function signatureOver(entryHash: string, key = BUYER): string {
  return `0x${toHex(key.sign(fromHex(entryHash)))}`
}

async function prepared(): Promise<PreparedPurchase> {
  stubSimulation(simulated([purchaseEntry()]))
  const purchase = await theSale().buy({ buyer: BUYER.publicKey(), amount: AMOUNT })
  if (!purchase.success) throw new Error(purchase.message)
  return purchase.data
}

function simulatedEntries(params: unknown): Array<xdr.SorobanAuthorizationEntry> {
  if (typeof params !== 'object' || params === null || !('transaction' in params) || typeof params.transaction !== 'string') {
    throw new Error('No transaction simulated')
  }
  const [operation] = new Transaction(params.transaction, Networks.TESTNET).operations
  if (operation?.type !== 'invokeHostFunction') throw new Error('No contract call simulated')
  return operation.auth ?? []
}

afterEach(() => vi.unstubAllGlobals())

describe('buy', () => {
  it('returns the one entry covering the purchase and the payment, with its hash and expiration ledger', async () => {
    const purchase = await prepared()
    const entry = xdr.SorobanAuthorizationEntry.fromXDR(purchase.entry, 'base64')
    const expectedHash = hash(buildAuthorizationEntryPreimage(entry, EXPIRATION_LEDGER, Networks.TESTNET).toXDR())

    expect(purchase).toMatchObject({
      chainId: CHAIN_ID.STELLAR_TESTNET,
      buyer: BUYER.publicKey(),
      amount: AMOUNT,
      expirationLedger: EXPIRATION_LEDGER,
      entryHash: toHex(expectedHash),
    })
    expect(entry.rootInvocation.subInvocations).toHaveLength(1)
  })

  it('refuses a simulation asking for more than one entry', async () => {
    stubSimulation(simulated([purchaseEntry(), purchaseEntry()]))

    expect(await theSale().buy({ buyer: BUYER.publicKey(), amount: AMOUNT })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.UNEXPECTED_AUTHORIZATION },
    })
  })

  it('refuses an entry that pays someone other than the sale', async () => {
    stubSimulation(simulated([purchaseEntry({ recipient: USDC })]))

    expect(await theSale().buy({ buyer: BUYER.publicKey(), amount: AMOUNT })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.UNEXPECTED_AUTHORIZATION },
    })
  })

  it('refuses a buyer that is not an account address, without asking the chain', async () => {
    const requests = stubSimulation(simulated([purchaseEntry()]))

    expect(await theSale().buy({ buyer: 'GNOTANADDRESS', amount: AMOUNT })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.INVALID_BUYER },
    })
    expect(requests).toEqual([])
  })

  it('refuses an entry another address would sign', async () => {
    stubSimulation(simulated([purchaseEntry({ signer: Keypair.random().publicKey() })]))

    expect(await theSale().buy({ buyer: BUYER.publicKey(), amount: AMOUNT })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.UNEXPECTED_AUTHORIZATION },
    })
  })

  it.each([
    [6100, SALE_REFUSAL.INVESTOR_NOT_ENABLED],
    [6101, SALE_REFUSAL.ALLOCATION_EXCEEDED],
    [6107, SALE_REFUSAL.TOO_MANY_RECEIPTS],
    [6201, SALE_REFUSAL.SALE_NOT_ACTIVE],
    [6206, SALE_REFUSAL.NOT_WHOLE_QUOTAS],
    [6207, SALE_REFUSAL.EXCEEDS_SUPPLY],
    [6220, SALE_REFUSAL.TOO_MANY_PURCHASES],
    [10, SALE_REFUSAL.INSUFFICIENT_BALANCE],
    [11, SALE_REFUSAL.BALANCE_FROZEN],
    [13, SALE_REFUSAL.TRUSTLINE_MISSING],
    [6229, SALE_REFUSAL.CONTRACT_REFUSED],
  ])('names contract error %i as %s', async (contractError, code) => {
    stubSimulation({ latestLedger: LATEST_LEDGER, error: `HostError: Error(Contract, #${contractError})`, events: [] })

    expect(await theSale().buy({ buyer: BUYER.publicKey(), amount: AMOUNT })).toMatchObject({
      success: false,
      error: { code },
    })
  })

  it('binds the entry hash to the network of the chain id', async () => {
    stubSimulation(simulated([purchaseEntry()]), Networks.PUBLIC)

    const purchase = await theSale(SALE, CHAIN_ID.STELLAR_MAINNET, NETWORK.MAINNET).buy({
      buyer: BUYER.publicKey(),
      amount: AMOUNT,
    })

    if (!purchase.success) throw new Error(purchase.message)
    const entry = xdr.SorobanAuthorizationEntry.fromXDR(purchase.data.entry, 'base64')
    const mainnetHash = hash(buildAuthorizationEntryPreimage(entry, EXPIRATION_LEDGER, Networks.PUBLIC).toXDR())
    expect(purchase.data).toMatchObject({ chainId: CHAIN_ID.STELLAR_MAINNET, entryHash: toHex(mainnetHash) })
  })

  it('refuses a client that serves another network than the chain id', async () => {
    stubSimulation(simulated([purchaseEntry()]))

    expect(await theSale(SALE, CHAIN_ID.STELLAR_MAINNET).buy({ buyer: BUYER.publicKey(), amount: AMOUNT })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.CHAIN_MISMATCH },
    })
  })

  it('refuses the same way when the client itself finds its endpoint on another network', async () => {
    stubSimulation(simulated([purchaseEntry()]))

    expect(
      await theSale(SALE, CHAIN_ID.STELLAR_MAINNET, NETWORK.MAINNET).buy({ buyer: BUYER.publicKey(), amount: AMOUNT }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.CHAIN_MISMATCH } })
  })

  it('says the chain is unavailable when no endpoint answers', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new Error('offline'))))

    expect(await theSale().buy({ buyer: BUYER.publicKey(), amount: AMOUNT })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.CHAIN_UNAVAILABLE },
    })
  })
})

describe('authorize', () => {
  it('refuses a signature made by another key', async () => {
    const purchase = await prepared()

    expect(
      await theSale().authorize({ prepared: purchase, signature: signatureOver(purchase.entryHash, Keypair.random()) }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.SIGNATURE_MISMATCH } })
  })

  it('refuses the buyer’s signature over another hash', async () => {
    const purchase = await prepared()

    expect(
      await theSale().authorize({ prepared: purchase, signature: signatureOver('ab'.repeat(32)) }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.SIGNATURE_MISMATCH } })
  })

  it('simulates the signed entry again', async () => {
    const purchase = await prepared()
    const requests = stubSimulation(simulated([]))

    const authorized = await theSale().authorize({ prepared: purchase, signature: signatureOver(purchase.entryHash) })

    const simulation = requests.find(({ method }) => method === 'simulateTransaction')
    const [entry] = simulatedEntries(simulation?.params)
    expect(authorized.success).toBe(true)
    expect(entry?.credentials.type).toBe('sorobanCredentialsAddressV2')
    expect(entry?.toXDR('base64')).not.toBe(purchase.entry)
  })

  it('assembles the transaction with the resources the signed simulation reports', async () => {
    const purchase = await prepared()
    stubSimulation(simulated([]))
    const authorized = await theSale().authorize({ prepared: purchase, signature: signatureOver(purchase.entryHash) })
    if (!authorized.success) throw new Error(authorized.message)
    const channel = Keypair.random()
    stubChain({ accounts: [accountEntry(channel.publicKey(), 7n)], simulated: simulated([], LATEST_LEDGER, '91234') })

    const assembled = await createRelay({ rpc: rpc(), sponsor: keypairSigner(Keypair.random()) }).prepareForSigners({
      source: channel.publicKey(),
      operations: [xdr.Operation.fromXDR(authorized.data.invocation, 'base64')],
      timeBound: 1_793_000_060,
    })

    if (!assembled.success) throw new Error(assembled.message)
    const transaction = new Transaction(assembled.data.envelope, Networks.TESTNET)
    const envelope = transaction.toEnvelope()
    const ext = envelope.type === 'envelopeTypeTx' ? envelope.value.tx.ext : undefined
    const [operation] = transaction.operations
    expect(ext?.type === 'sorobanData' ? String(ext.value.resourceFee) : undefined).toBe('91234')
    expect(operation?.type === 'invokeHostFunction' ? operation.auth?.length : undefined).toBe(1)
  })

  it('refuses an entry whose expiration ledger has been reached', async () => {
    const purchase = await prepared()
    stubSimulation(simulated([], EXPIRATION_LEDGER))

    expect(
      await theSale().authorize({ prepared: purchase, signature: signatureOver(purchase.entryHash) }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.ENTRY_EXPIRED } })
  })

  it('names the contract’s refusal of the signed call', async () => {
    const purchase = await prepared()
    stubSimulation({ latestLedger: LATEST_LEDGER, error: 'HostError: Error(Contract, #6207)', events: [] })

    expect(
      await theSale().authorize({ prepared: purchase, signature: signatureOver(purchase.entryHash) }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.EXCEEDS_SUPPLY } })
  })
})

const TREASURY = Keypair.random().publicKey()

function closeEntry({ caller = TREASURY, signer = TREASURY }: { caller?: string; signer?: string } = {}) {
  return unsignedEntry(
    signer,
    new xdr.SorobanAuthorizedInvocation({ function: contractCall(SALE, 'close', [scv.address(caller)]), subInvocations: [] }),
  )
}

const LATEST_CLOSE_TIME = 1_795_000_000
const LEDGER_MS = 5_000
const MAX_ENTRY_TTL = 3_110_400
const VALID_UNTIL = (LATEST_CLOSE_TIME + 1_800) * 1_000

function stubCloseChain(simulation: Record<string, unknown>, { reportsSettings = true } = {}) {
  const scpTiming = xdr.LedgerEntryData.configSetting(
    xdr.ConfigSettingEntry.configSettingScpTiming(
      new xdr.ConfigSettingScpTiming({
        ledgerTargetCloseTimeMilliseconds: LEDGER_MS,
        nominationTimeoutInitialMilliseconds: 1_000,
        nominationTimeoutIncrementMilliseconds: 1_000,
        ballotTimeoutInitialMilliseconds: 1_000,
        ballotTimeoutIncrementMilliseconds: 1_000,
      }),
    ),
  )
  const stateArchival = xdr.LedgerEntryData.configSetting(
    xdr.ConfigSettingEntry.configSettingStateArchival(
      new xdr.StateArchivalSettings({
        maxEntryTtl: MAX_ENTRY_TTL,
        minTemporaryTtl: 16,
        minPersistentTtl: 120_960,
        persistentRentRateDenominator: 1n,
        tempRentRateDenominator: 1n,
        maxEntriesToArchive: 1_000,
        liveSorobanStateSizeWindowSampleSize: 30,
        liveSorobanStateSizeWindowSamplePeriod: 64,
        evictionScanSize: 100_000,
        startingEvictionScanLevel: 7,
      }),
    ),
  )
  return stubJsonRpc(({ method }) => {
    if (method === 'getNetwork') return { passphrase: Networks.TESTNET, protocolVersion: 23 }
    if (method === 'getLatestLedger') {
      return { id: 'ab'.repeat(32), sequence: LATEST_LEDGER, protocolVersion: 23, closeTime: String(LATEST_CLOSE_TIME) }
    }
    if (method === 'getLedgerEntries') {
      return {
        entries: (reportsSettings ? [
          { configSettingId: xdr.ConfigSettingId.configSettingScpTiming, entry: scpTiming },
          { configSettingId: xdr.ConfigSettingId.configSettingStateArchival, entry: stateArchival },
        ] : []).map(({ configSettingId, entry }) => ({
          key: xdr.LedgerKey.configSetting(new xdr.LedgerKeyConfigSetting({ configSettingId })).toXDR('base64'),
          xdr: entry.toXDR('base64'),
          lastModifiedLedgerSeq: 1,
        })),
        latestLedger: LATEST_LEDGER,
      }
    }
    return simulation
  })
}

describe('close', () => {
  it('returns the one entry the treasury signs to close the sale, with its hash and expiration ledger', async () => {
    stubCloseChain(simulated([closeEntry()]))

    const closing = await theSale().close({ treasury: TREASURY, validUntil: VALID_UNTIL })

    if (!closing.success) throw new Error(closing.message)
    const entry = xdr.SorobanAuthorizationEntry.fromXDR(closing.data.entry, 'base64')
    const expectedHash = hash(buildAuthorizationEntryPreimage(entry, EXPIRATION_LEDGER, Networks.TESTNET).toXDR())
    expect(closing.data).toMatchObject({
      chainId: CHAIN_ID.STELLAR_TESTNET,
      treasury: TREASURY,
      expirationLedger: EXPIRATION_LEDGER,
      entryHash: toHex(expectedHash),
    })
  })

  it.each([
    ['closes for another caller', closeEntry({ caller: Keypair.random().publicKey() })],
    ['another address would sign', closeEntry({ signer: Keypair.random().publicKey() })],
    ['covers a purchase', purchaseEntry({ signer: TREASURY })],
  ])('refuses an entry that %s', async (_, entry) => {
    stubCloseChain(simulated([entry]))

    expect(await theSale().close({ treasury: TREASURY, validUntil: VALID_UNTIL })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.UNEXPECTED_AUTHORIZATION },
    })
  })

  it('refuses a treasury that is not an account address, without asking the chain', async () => {
    const requests = stubCloseChain(simulated([closeEntry()]))

    expect(await theSale().close({ treasury: SALE, validUntil: VALID_UNTIL })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.INVALID_TREASURY },
    })
    expect(requests).toEqual([])
  })

  it.each([
    [6228, SALE_REFUSAL.BELOW_MINIMUM],
    [6201, SALE_REFUSAL.SALE_NOT_ACTIVE],
  ])('names contract error %i as %s', async (contractError, code) => {
    stubCloseChain({ latestLedger: LATEST_LEDGER, error: `HostError: Error(Contract, #${contractError})`, events: [] })

    expect(await theSale().close({ treasury: TREASURY, validUntil: VALID_UNTIL })).toMatchObject({ success: false, error: { code } })
  })

  it.each([
    ['a moment between two ledgers, rounded up to the next ledger', VALID_UNTIL - 1, EXPIRATION_LEDGER],
    ['the latest moment the network allows', (LATEST_CLOSE_TIME + (MAX_ENTRY_TTL - 1) * 5) * 1_000, LATEST_LEDGER + MAX_ENTRY_TTL - 1],
  ])('keeps the entry valid until %s', async (_, validUntil, expirationLedger) => {
    stubCloseChain(simulated([closeEntry()]))

    expect(await theSale().close({ treasury: TREASURY, validUntil })).toMatchObject({ success: true, data: { expirationLedger } })
  })

  it.each([
    ['already past', LATEST_CLOSE_TIME * 1_000, SALE_REFUSAL.VALID_UNTIL_PASSED],
    ['beyond what the network allows', (LATEST_CLOSE_TIME + MAX_ENTRY_TTL * 5) * 1_000, SALE_REFUSAL.VALID_UNTIL_TOO_LATE],
  ])('refuses a moment %s', async (_, validUntil, code) => {
    stubCloseChain(simulated([closeEntry()]))

    expect(await theSale().close({ treasury: TREASURY, validUntil })).toMatchObject({ success: false, error: { code } })
  })

  it('refuses a moment the chain reached while the close was being prepared', async () => {
    stubCloseChain(simulated([closeEntry()], EXPIRATION_LEDGER))

    expect(await theSale().close({ treasury: TREASURY, validUntil: VALID_UNTIL })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.VALID_UNTIL_PASSED },
    })
  })

  it('says the response was unexpected when the network does not report its ledger time and entry lifetime', async () => {
    stubCloseChain(simulated([closeEntry()]), { reportsSettings: false })

    expect(await theSale().close({ treasury: TREASURY, validUntil: VALID_UNTIL })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.UNEXPECTED_RESPONSE },
    })
  })
})

describe('authorizeClose', () => {
  const officer = (seed: number) => Keypair.fromRawEd25519Seed(new Uint8Array(32).fill(seed))
  const FIRST = officer(3)
  const SECOND = officer(9)
  const THIRD = officer(1)
  const OFFICERS = [FIRST, SECOND, THIRD]

  async function preparedClose() {
    stubCloseChain(simulated([closeEntry()]))
    const closing = await theSale().close({ treasury: TREASURY, validUntil: VALID_UNTIL })
    if (!closing.success) throw new Error(closing.message)
    return closing.data
  }

  function stubTreasury(simulation: Record<string, unknown> = simulated([])) {
    return stubChain({
      accounts: [
        accountEntry(TREASURY, 1n, { thresholds: [0, 2, 2, 2], signers: OFFICERS.map((signer) => [signer.publicKey(), 1]) }),
      ],
      simulated: simulation,
    })
  }

  function signedBy(entryHash: string, officers: Array<Keypair>) {
    return officers.map((officer) => ({ address: officer.publicKey(), signature: signatureOver(entryHash, officer) }))
  }

  it('refuses fewer officers than the treasury’s threshold, without simulating', async () => {
    const closing = await preparedClose()
    const requests = stubTreasury()

    expect(await theSale().authorizeClose({ prepared: closing, signatures: signedBy(closing.entryHash, [FIRST]) })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.BELOW_THRESHOLD },
    })
    expect(requests.map(({ method }) => method)).not.toContain('simulateTransaction')
  })

  it('refuses signatures over a close whose expiration was changed after it was prepared, without asking the chain', async () => {
    const closing = await preparedClose()
    const requests = stubTreasury()

    expect(
      await theSale().authorizeClose({
        prepared: { ...closing, expirationLedger: closing.expirationLedger + 1 },
        signatures: signedBy(closing.entryHash, [FIRST, SECOND]),
      }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.SIGNATURE_MISMATCH } })
    expect(requests).toEqual([])
  })

  it.each([
    ['another call', () => purchaseEntry({ signer: TREASURY }).toXDR('base64')],
    ['another address', () => closeEntry({ signer: Keypair.random().publicKey() }).toXDR('base64')],
    ['no entry at all', () => 'bm90IHhkcg=='],
  ])('refuses a prepared close whose entry was swapped for %s, without asking the chain', async (_, entry) => {
    const closing = await preparedClose()
    const requests = stubTreasury()

    expect(
      await theSale().authorizeClose({ prepared: { ...closing, entry: entry() }, signatures: signedBy(closing.entryHash, [FIRST, SECOND]) }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.UNEXPECTED_AUTHORIZATION } })
    expect(requests).toEqual([])
  })

  it('counts an officer who signed twice once', async () => {
    const closing = await preparedClose()
    stubTreasury()

    expect(
      await theSale().authorizeClose({ prepared: closing, signatures: signedBy(closing.entryHash, [FIRST, FIRST]) }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.BELOW_THRESHOLD } })
  })

  it.each([
    ['made over another hash', () => signedBy('ab'.repeat(32), [FIRST, SECOND])],
    ['made by someone who is not an officer', (entryHash: string) => signedBy(entryHash, [FIRST, SECOND, Keypair.random()])],
  ])('refuses a signature %s', async (_, signatures) => {
    const closing = await preparedClose()
    stubTreasury()

    expect(await theSale().authorizeClose({ prepared: closing, signatures: signatures(closing.entryHash) })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.SIGNATURE_MISMATCH },
    })
  })

  it('signs the entry with the officers’ signatures in raw public key order and simulates it again', async () => {
    const closing = await preparedClose()
    const requests = stubTreasury()

    const authorized = await theSale().authorizeClose({ prepared: closing, signatures: signedBy(closing.entryHash, OFFICERS) })

    if (!authorized.success) throw new Error(authorized.message)
    const [entry] = simulatedEntries(requests.find(({ method }) => method === 'simulateTransaction')?.params)
    if (entry?.credentials.type !== 'sorobanCredentialsAddressV2') throw new Error('No address entry simulated')
    const { signature, signatureExpirationLedger } = entry.credentials.addressV2
    const signers: Array<{ public_key: Uint8Array }> = scValToNative(signature)
    expect(signers.map(({ public_key }) => toHex(public_key))).toEqual([THIRD, FIRST, SECOND].map((key) => toHex(key.rawPublicKey())))
    expect(signatureExpirationLedger).toBe(closing.expirationLedger)
    const relayed = xdr.Operation.fromXDR(authorized.data.invocation, 'base64').body
    if (relayed.type !== 'invokeHostFunction') throw new Error('No contract call relayed')
    expect(relayed.invokeHostFunctionOp.auth.map((signed) => signed.toXDR('base64'))).toEqual([entry.toXDR('base64')])
  })

  it('refuses an entry whose expiration ledger has been reached', async () => {
    const closing = await preparedClose()
    stubTreasury(simulated([], EXPIRATION_LEDGER))

    expect(
      await theSale().authorizeClose({ prepared: closing, signatures: signedBy(closing.entryHash, [FIRST, SECOND]) }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.ENTRY_EXPIRED } })
  })

  it('names the contract’s refusal of the signed close', async () => {
    const closing = await preparedClose()
    stubTreasury({ latestLedger: LATEST_LEDGER, error: 'HostError: Error(Contract, #6228)', events: [] })

    expect(
      await theSale().authorizeClose({ prepared: closing, signatures: signedBy(closing.entryHash, [FIRST, SECOND]) }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.BELOW_MINIMUM } })
  })
})

describe('purchaseReceipt', () => {
  const transaction = { chainId: CHAIN_ID.STELLAR_TESTNET, resultMeta: RECEIPT }

  it('reads the quotas and the amount paid from the purchase event', () => {
    expect(theSale().purchaseReceipt({ buyer: RECEIPT_BUYER, transaction })).toMatchObject({
      success: true,
      data: { quotas: 3n, paid: 300_000_000n },
    })
  })

  it('finds no purchase by another buyer', () => {
    expect(theSale().purchaseReceipt({ buyer: BUYER.publicKey(), transaction })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.PURCHASE_NOT_FOUND },
    })
  })

  it('ignores a purchase event another contract emitted', () => {
    expect(theSale(USDC).purchaseReceipt({ buyer: RECEIPT_BUYER, transaction })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.PURCHASE_NOT_FOUND },
    })
  })

  it('says so when the transaction cannot be read, rather than reporting no purchase', () => {
    expect(
      theSale().purchaseReceipt({ buyer: RECEIPT_BUYER, transaction: { chainId: CHAIN_ID.STELLAR_TESTNET, resultMeta: 'bm90IHhkcg==' } }),
    ).toMatchObject({ success: false, error: { code: SALE_REFUSAL.UNREADABLE_TRANSACTION } })
  })
})

describe('read', () => {
  function answerFor(params: unknown, state = 'Active'): xdr.ScVal {
    if (typeof params !== 'object' || params === null || !('transaction' in params) || typeof params.transaction !== 'string') {
      throw new Error('No transaction simulated')
    }
    const [operation] = new Transaction(params.transaction, Networks.TESTNET).operations
    if (operation?.type !== 'invokeHostFunction' || operation.func.type !== 'hostFunctionTypeInvokeContract') {
      throw new Error('No contract call simulated')
    }
    const method = operation.func.invokeContract.functionName.toString()
    if (method === 'state') return xdr.ScVal.scvVec([xdr.ScVal.scvSymbol(state)])
    if (method === 'finalizable_at') return scv.u64(1_793_086_401n)
    if (method === 'config') return scv.struct({ price: scv.i128(100_000_000n) })
    if (method === 'accounting') {
      return scv.struct({ supply: scv.i128(1_000n), sold_tokens: scv.i128(3n), raised: scv.i128(300_000_000n) })
    }
    return scv.struct({ payment_asset: nativeToScVal(USDC, { type: 'address' }) })
  }

  function stubSale(state?: string) {
    stubJsonRpc(({ method, params }) =>
      method === 'getNetwork'
        ? { passphrase: Networks.TESTNET, protocolVersion: 23 }
        : {
            latestLedger: LATEST_LEDGER,
            minResourceFee: '100',
            transactionData: new SorobanDataBuilder().build().toXDR('base64'),
            results: [{ xdr: answerFor(params, state).toXDR('base64'), auth: [] }],
            events: [],
          },
    )
  }

  it('reports the state, price and accounting of the sale in chain-neutral terms', async () => {
    stubSale()

    expect(await theSale().read()).toMatchObject({
      success: true,
      data: {
        state: SALE_STATE.ACTIVE,
        pricePerQuota: 100_000_000n,
        supply: 1_000n,
        soldQuotas: 3n,
        raised: 300_000_000n,
        paymentAsset: USDC,
      },
    })
  })

  it('reports when a closing sale can be finalized and its distribution begin', async () => {
    stubSale('Closing')

    expect(await theSale().read()).toMatchObject({
      success: true,
      data: { state: SALE_STATE.CLOSING, finalizableAt: 1_793_086_401_000 },
    })
  })

  it('reports no finalization time while the sale is still active', async () => {
    stubSale()

    const sale = await theSale().read()

    if (!sale.success) throw new Error(sale.message)
    expect(sale.data).not.toHaveProperty('finalizableAt')
  })

  it('says the response was unexpected when the address does not answer like a sale', async () => {
    stubSimulation(simulated([]))

    expect(await theSale().read()).toMatchObject({ success: false, error: { code: SALE_REFUSAL.UNEXPECTED_RESPONSE } })
  })

  it('names the contract’s refusal of the read', async () => {
    stubSimulation({ latestLedger: LATEST_LEDGER, error: 'HostError: Error(Contract, #6201)', events: [] })

    expect(await theSale().read()).toMatchObject({ success: false, error: { code: SALE_REFUSAL.SALE_NOT_ACTIVE } })
  })
})

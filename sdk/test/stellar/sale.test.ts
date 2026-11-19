import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import {
  Address,
  buildAuthorizationEntryPreimage,
  hash,
  Keypair,
  nativeToScVal,
  Networks,
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

function purchaseEntry({
  recipient = SALE,
  buyer = BUYER.publicKey(),
  signer = BUYER.publicKey(),
}: { recipient?: string; buyer?: string; signer?: string } = {}) {
  return new xdr.SorobanAuthorizationEntry({
    credentials: xdr.SorobanCredentials.sorobanCredentialsAddressV2(
      new xdr.SorobanAddressCredentials({
        address: Address.fromString(signer).toScAddress(),
        nonce: 7n,
        signatureExpirationLedger: 0,
        signature: xdr.ScVal.scvVoid(),
      }),
    ),
    rootInvocation: new xdr.SorobanAuthorizedInvocation({
      function: contractCall(SALE, 'buy', [scv.address(buyer), scv.i128(AMOUNT)]),
      subInvocations: [
        new xdr.SorobanAuthorizedInvocation({
          function: contractCall(USDC, 'transfer', [scv.address(buyer), scv.address(recipient), scv.i128(AMOUNT)]),
          subInvocations: [],
        }),
      ],
    }),
  })
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
  function answerFor(params: unknown): xdr.ScVal {
    if (typeof params !== 'object' || params === null || !('transaction' in params) || typeof params.transaction !== 'string') {
      throw new Error('No transaction simulated')
    }
    const [operation] = new Transaction(params.transaction, Networks.TESTNET).operations
    if (operation?.type !== 'invokeHostFunction' || operation.func.type !== 'hostFunctionTypeInvokeContract') {
      throw new Error('No contract call simulated')
    }
    const method = operation.func.invokeContract.functionName.toString()
    if (method === 'state') return xdr.ScVal.scvVec([xdr.ScVal.scvSymbol('Active')])
    if (method === 'config') return scv.struct({ price: scv.i128(100_000_000n) })
    if (method === 'accounting') {
      return scv.struct({ supply: scv.i128(1_000n), sold_tokens: scv.i128(3n), raised: scv.i128(300_000_000n) })
    }
    return scv.struct({ payment_asset: nativeToScVal(USDC, { type: 'address' }) })
  }

  it('reports the state, price and accounting of the sale in chain-neutral terms', async () => {
    stubJsonRpc(({ method, params }) =>
      method === 'getNetwork'
        ? { passphrase: Networks.TESTNET, protocolVersion: 23 }
        : {
            latestLedger: LATEST_LEDGER,
            minResourceFee: '100',
            transactionData: new SorobanDataBuilder().build().toXDR('base64'),
            results: [{ xdr: answerFor(params).toXDR('base64'), auth: [] }],
            events: [],
          },
    )

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

  it('says the response was unexpected when the address does not answer like a sale', async () => {
    stubSimulation(simulated([]))

    expect(await theSale().read()).toMatchObject({ success: false, error: { code: SALE_REFUSAL.UNEXPECTED_RESPONSE } })
  })

  it('names the contract’s refusal of the read', async () => {
    stubSimulation({ latestLedger: LATEST_LEDGER, error: 'HostError: Error(Contract, #6201)', events: [] })

    expect(await theSale().read()).toMatchObject({ success: false, error: { code: SALE_REFUSAL.SALE_NOT_ACTIVE } })
  })
})

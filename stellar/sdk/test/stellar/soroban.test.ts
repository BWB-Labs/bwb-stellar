import {
  Account,
  BASE_FEE,
  hash,
  Keypair,
  Networks,
  SorobanDataBuilder,
  TransactionBuilder,
  xdr,
} from '@stellar/stellar-sdk/base'
import { Server } from '@stellar/stellar-sdk/rpc'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  firstRequestedKey,
  stubJsonRpc,
} from './support/chain.js'
import {
  contractAddressOf,
  contractExists,
  contractSalt,
  createContractOperation,
  invokeContractOperation,
  invokesHostFunction,
  prepareSorobanTransaction,
  scv,
  simulateRead,
  SorobanSimulationError,
} from '../../src/stellar/soroban.js'

const TESTNET_DEPLOYER =
  'GABDEXIHWESSJFO64VR2GUVZ25JDTTNWEMF3EFPC7Z5I5QMVOFQDYENI'
const TOKEN_SALT =
  '75faa3f5834618ebf8a18dee19a1c8e2d7059dd0a37f5fe65eddc0c397596e0b'
const SALE_SALT =
  '6b1ec037ec4729e8c32125944868da861b20d2371fa478a9c01ddd78727291e6'
const TOKEN = 'CBX74ZZUBLYPBNMND7DWTU74I4QWUIQEJZNH2XJVBX3THZ2KDRRFMESM'
const SALE = 'CCL2OOMX4NYI4Y6RDYHFRJ6VKNKLVM7POOZIMKJUSJRUTETDKAIPK5IT'

afterEach(() => {
  vi.unstubAllGlobals()
})

function hex(salt: string): Uint8Array {
  return Uint8Array.from(Buffer.from(salt, 'hex'))
}

describe('contractSalt', () => {
  it('derives the salt the contracts repository derives for the same offering label', () => {
    expect(
      Buffer.from(contractSalt('bwb:l2-demo-3:token')).toString('hex'),
    ).toBe(TOKEN_SALT)
    expect(
      Buffer.from(contractSalt('bwb:l2-demo-3:sale')).toString('hex'),
    ).toBe(SALE_SALT)
  })
})

describe('contractAddressOf', () => {
  it('computes the addresses the testnet token and sale were deployed at from the deployer and the salts', () => {
    const networkId = hash(Networks.TESTNET)

    expect(
      contractAddressOf({
        deployer: TESTNET_DEPLOYER,
        salt: hex(TOKEN_SALT),
        networkId,
      }),
    ).toBe(TOKEN)
    expect(
      contractAddressOf({
        deployer: TESTNET_DEPLOYER,
        salt: hex(SALE_SALT),
        networkId,
      }),
    ).toBe(SALE)
  })

  it('gives another deployer another address for the same salt', () => {
    expect(
      contractAddressOf({
        deployer: Keypair.random().publicKey(),
        salt: hex(TOKEN_SALT),
        networkId: hash(Networks.TESTNET),
      }),
    ).not.toBe(TOKEN)
  })
})

describe('invokesHostFunction', () => {
  it('recognizes a contract creation and a contract call, sourced by the deployer', () => {
    const creation = createContractOperation({
      deployer: TESTNET_DEPLOYER,
      salt: hex(TOKEN_SALT),
      wasmHash: hex(TOKEN_SALT),
      constructorArgs: [scv.u32(1)],
    })
    const call = invokeContractOperation({
      contract: TOKEN,
      method: 'activate',
      args: [],
      source: TESTNET_DEPLOYER,
    })

    expect(invokesHostFunction(creation)).toBe(true)
    expect(invokesHostFunction(call)).toBe(true)
    expect(
      creation.sourceAccount?.type === 'keyTypeEd25519'
        ? creation.sourceAccount.ed25519.toString()
        : null,
    ).toBe(
      Buffer.from(
        Keypair.fromPublicKey(TESTNET_DEPLOYER).rawPublicKey(),
      ).toString('hex'),
    )
  })
})

describe('scv.struct', () => {
  it('builds a map keyed by sorted symbols, as a contract struct is encoded', () => {
    const struct = scv.struct({
      symbol: scv.string('TT'),
      name: scv.string('Test'),
    })

    expect(
      struct.type === 'scvMap'
        ? struct.map?.map(({ key }) =>
            key.type === 'scvSymbol' ? key.sym.toString() : null,
          )
        : null,
    ).toEqual(['name', 'symbol'])
  })
})

function sorobanData(): string {
  return new SorobanDataBuilder().build().toXDR('base64')
}

describe('prepareSorobanTransaction', () => {
  async function prepared(resourceFee: string) {
    stubJsonRpc(() => ({
      latestLedger: 7,
      minResourceFee: resourceFee,
      transactionData: new SorobanDataBuilder()
        .setResourceFee(resourceFee)
        .build()
        .toXDR('base64'),
      results: [{ xdr: xdr.ScVal.scvVoid().toXDR('base64'), auth: [] }],
      events: [],
    }))
    const transaction = new TransactionBuilder(
      new Account(TESTNET_DEPLOYER, '1'),
      { fee: BASE_FEE, networkPassphrase: Networks.TESTNET },
    )
      .addOperation(
        invokeContractOperation({
          contract: SALE,
          method: 'activate',
          args: [],
          source: TESTNET_DEPLOYER,
        }),
      )
      .setTimeout(30)
      .build()
    const built = await prepareSorobanTransaction(
      new Server('https://rpc.example'),
      transaction,
    )
    const { ext } = built.toEnvelope().value.tx
    return {
      fee: built.fee,
      resourceFee:
        ext.type === 'sorobanData' ? String(ext.value.resourceFee) : null,
    }
  }

  it('charges the simulated resource fee on top of the base fee when the sum fits the transaction fee field', async () => {
    expect(await prepared('500')).toEqual({ fee: '600', resourceFee: '500' })
  })

  it('declares a resource fee above the transaction fee field’s 32-bit limit and keeps only the base fee in that field, for a fee-bump to pay', async () => {
    expect(await prepared('13440000000')).toEqual({
      fee: '100',
      resourceFee: '13440000000',
    })
  })
})

describe('simulateRead', () => {
  it('returns the decoded value the contract call would return', async () => {
    stubJsonRpc(({ method }) =>
      method === 'simulateTransaction'
        ? {
            latestLedger: 7,
            minResourceFee: '100',
            transactionData: sorobanData(),
            results: [{ xdr: scv.u32(24321).toXDR('base64'), auth: [] }],
            events: [],
          }
        : {},
    )

    const period = await simulateRead(
      new Server('https://rpc.example'),
      Networks.TESTNET,
      {
        source: TESTNET_DEPLOYER,
        operation: invokeContractOperation({
          contract: TOKEN,
          method: 'current_period',
          args: [],
          source: TESTNET_DEPLOYER,
        }),
      },
    )

    expect(period).toBe(24321)
  })

  it('throws the simulation error when the contract call fails', async () => {
    stubJsonRpc(() => ({
      latestLedger: 7,
      error: 'HostError: Error(Contract, #6200)',
      events: [],
    }))

    await expect(
      simulateRead(new Server('https://rpc.example'), Networks.TESTNET, {
        source: TESTNET_DEPLOYER,
        operation: invokeContractOperation({
          contract: SALE,
          method: 'activate',
          args: [],
          source: TESTNET_DEPLOYER,
        }),
      }),
    ).rejects.toThrow(SorobanSimulationError)
  })
})

describe('contractExists', () => {
  it('reads the contract instance entry at the address', async () => {
    const requests = stubJsonRpc(({ params }) => ({
      entries: [
        {
          key: firstRequestedKey(params),
          xdr: xdr.LedgerEntryData.contractData(
            new xdr.ContractDataEntry({
              ext: xdr.ExtensionPoint.v0(),
              contract: xdr.ScAddress.scAddressTypeContract(
                new xdr.ContractId(new Uint8Array(32)),
              ),
              key: xdr.ScVal.scvLedgerKeyContractInstance(),
              durability: xdr.ContractDataDurability.persistent,
              val: xdr.ScVal.scvVoid(),
            }),
          ).toXDR('base64'),
          lastModifiedLedgerSeq: 1,
        },
      ],
      latestLedger: 7,
    }))

    const exists = await contractExists(
      new Server('https://rpc.example'),
      TOKEN,
    )

    expect(exists).toBe(true)
    expect(requests.map(({ method }) => method)).toEqual(['getLedgerEntries'])
  })

  it('says the contract is absent when the network holds no instance entry', async () => {
    stubJsonRpc(() => ({ entries: [], latestLedger: 7 }))

    expect(await contractExists(new Server('https://rpc.example'), SALE)).toBe(
      false,
    )
  })
})

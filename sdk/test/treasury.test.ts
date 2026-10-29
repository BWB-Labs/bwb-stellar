import { Asset, Keypair, Operation, type xdr } from '@stellar/stellar-sdk/base'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { connect, NETWORK } from '../src/network.js'
import { readSignerSet, signerChangeOperations, treasuryCreationOperations } from '../src/treasury.js'
import { accountEntry, RPC_URL, stubChain } from './support/chain.js'

const TREASURY = Keypair.random().publicKey()
const SPONSOR = Keypair.random().publicKey()
const USDC = new Asset('USDC', Keypair.random().publicKey())
const [ANA, DIEGO, CARLA] = [Keypair.random(), Keypair.random(), Keypair.random()].map((key) => key.publicKey())

function decoded(operations: Array<xdr.Operation>) {
  return operations.map((operation) => Operation.fromXDRObject(operation))
}

afterEach(() => vi.unstubAllGlobals())

describe('treasuryCreationOperations', () => {
  it('creates the account in one sponsored transaction with a USDC trustline, every officer a signer and the master key disabled', () => {
    const operations = decoded(
      treasuryCreationOperations({ treasury: TREASURY, sponsor: SPONSOR, asset: USDC, signers: [ANA!, DIEGO!], threshold: 2 }),
    )

    expect(operations).toMatchObject([
      { type: 'beginSponsoringFutureReserves', sponsoredId: TREASURY, source: SPONSOR },
      { type: 'createAccount', destination: TREASURY, startingBalance: '0.0000000', source: SPONSOR },
      { type: 'changeTrust', line: { code: 'USDC' }, source: TREASURY },
      { type: 'setOptions', signer: { ed25519PublicKey: ANA, weight: 1 }, source: TREASURY },
      {
        type: 'setOptions',
        signer: { ed25519PublicKey: DIEGO, weight: 1 },
        masterWeight: 0,
        lowThreshold: 2,
        medThreshold: 2,
        highThreshold: 2,
        source: TREASURY,
      },
      { type: 'endSponsoringFutureReserves', source: TREASURY },
    ])
  })
})

describe('signerChangeOperations', () => {
  it('adds Carla under sponsorship and sets every threshold to 3', () => {
    const operations = decoded(
      signerChangeOperations({
        treasury: TREASURY,
        sponsor: SPONSOR,
        current: { signers: [ANA!, DIEGO!], threshold: 2 },
        target: { signers: [ANA!, DIEGO!, CARLA!], threshold: 3 },
      }),
    )

    expect(operations).toMatchObject([
      { type: 'beginSponsoringFutureReserves', sponsoredId: TREASURY, source: SPONSOR },
      { type: 'setOptions', signer: { ed25519PublicKey: CARLA, weight: 1 }, source: TREASURY },
      { type: 'endSponsoringFutureReserves', source: TREASURY },
      { type: 'setOptions', masterWeight: 0, lowThreshold: 3, medThreshold: 3, highThreshold: 3, source: TREASURY },
    ])
  })

  it('removes Diego without asking for sponsorship', () => {
    const operations = decoded(
      signerChangeOperations({
        treasury: TREASURY,
        sponsor: SPONSOR,
        current: { signers: [ANA!, DIEGO!], threshold: 2 },
        target: { signers: [ANA!], threshold: 1 },
      }),
    )

    expect(operations).toMatchObject([
      { type: 'setOptions', signer: { ed25519PublicKey: DIEGO, weight: 0 }, source: TREASURY },
      { type: 'setOptions', masterWeight: 0, lowThreshold: 1, medThreshold: 1, highThreshold: 1, source: TREASURY },
    ])
  })
})

describe('readSignerSet', () => {
  it('reads the officers and the threshold of a multisig whose master key is disabled', async () => {
    stubChain({ accounts: [accountEntry(TREASURY, 1n, { thresholds: [0, 2, 2, 2], signers: [[ANA!, 1], [DIEGO!, 1]] })] })

    const read = await readSignerSet(connect({ network: NETWORK.TESTNET, rpcUrls: [RPC_URL] }), TREASURY)

    expect(read?.threshold).toBe(2)
    expect(read?.signers.sort()).toEqual([ANA, DIEGO].sort())
  })

  it('counts the account’s own key while its master key still signs', async () => {
    stubChain({ accounts: [accountEntry(TREASURY, 1n)] })

    expect(await readSignerSet(connect({ network: NETWORK.TESTNET, rpcUrls: [RPC_URL] }), TREASURY)).toEqual({
      signers: [TREASURY],
      threshold: 0,
    })
  })

  it('answers nothing for an account that does not exist', async () => {
    stubChain({})

    expect(await readSignerSet(connect({ network: NETWORK.TESTNET, rpcUrls: [RPC_URL] }), TREASURY)).toBeNull()
  })
})

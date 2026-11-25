import { describe, expect, expectTypeOf, it } from 'vitest'

import { CHAIN_ID, createBwbSdk, SALE_REFUSAL, type ChainId, type Result, type SaleRefusal, type StellarRpc } from '../src/index.js'

const SALE = 'CDN3QJI4KSAULT4I7TQ5PO5SBPDPW3VIUDOAGUFVHUHKLLEZZUYHV4AC'
const rpc: StellarRpc = () => Promise.reject(new Error('No chain behind this client'))
const bwb = createBwbSdk({ chains: { [CHAIN_ID.STELLAR_TESTNET]: { rpc } } })

function outcomeOf(result: Result<unknown, SaleRefusal>) {
  return result.success ? 'found' : result.error.code
}

describe('select', () => {
  it('returns a client bound to a configured Stellar chain, whose sale needs only the address', () => {
    const client = bwb.select(CHAIN_ID.STELLAR_TESTNET)

    if (!client.success) throw new Error(client.message)
    expect(client.data.sale({ address: SALE })).toMatchObject({
      buy: expect.any(Function),
      authorize: expect.any(Function),
      purchaseReceipt: expect.any(Function),
      read: expect.any(Function),
    })
  })

  it('refuses a supported chain the configuration leaves out', () => {
    expect(bwb.select(CHAIN_ID.STELLAR_MAINNET)).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.CHAIN_NOT_CONFIGURED },
    })
  })

  it.each([CHAIN_ID.BASE_TESTNET, CHAIN_ID.BASE_MAINNET])('refuses the numeric chain id %i, which has no strategy yet', (chainId) => {
    expect(bwb.select(chainId)).toMatchObject({ success: false, error: { code: SALE_REFUSAL.CHAIN_NOT_SUPPORTED } })
  })

  it('takes every native chain id, numeric ones included', () => {
    expectTypeOf(bwb.select).parameter(0).toEqualTypeOf<ChainId>()
    expectTypeOf(bwb.select).toBeCallableWith(84532)
  })
})

describe('sale', () => {
  it.each(Object.values(CHAIN_ID))('answers for chain %s as select followed by sale does', (chainId) => {
    expect(outcomeOf(bwb.sale({ chainId, address: SALE }))).toBe(outcomeOf(bwb.select(chainId)))
  })
})

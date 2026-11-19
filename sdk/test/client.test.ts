import { describe, expect, it } from 'vitest'

import { CHAIN_ID, createBwbSdk, SALE_REFUSAL, type StellarRpc } from '../src/index.js'

const SALE = 'CDN3QJI4KSAULT4I7TQ5PO5SBPDPW3VIUDOAGUFVHUHKLLEZZUYHV4AC'
const rpc: StellarRpc = () => Promise.reject(new Error('No chain behind this client'))
const bwb = createBwbSdk({ chains: { [CHAIN_ID.STELLAR_TESTNET]: { rpc } } })

describe('createBwbSdk', () => {
  it('returns the sale of an offering on a configured Stellar chain', () => {
    expect(bwb.sale({ chainId: CHAIN_ID.STELLAR_TESTNET, address: SALE })).toMatchObject({
      success: true,
      data: {
        buy: expect.any(Function),
        authorize: expect.any(Function),
        outcome: expect.any(Function),
        read: expect.any(Function),
      },
    })
  })

  it('refuses a supported chain the configuration leaves out', () => {
    expect(bwb.sale({ chainId: CHAIN_ID.STELLAR_MAINNET, address: SALE })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.CHAIN_NOT_CONFIGURED },
    })
  })

  it.each([CHAIN_ID.BASE_TESTNET, CHAIN_ID.BASE_MAINNET])('refuses %s, which has no strategy yet', (chainId) => {
    expect(bwb.sale({ chainId, address: SALE })).toMatchObject({
      success: false,
      error: { code: SALE_REFUSAL.CHAIN_NOT_SUPPORTED },
    })
  })
})

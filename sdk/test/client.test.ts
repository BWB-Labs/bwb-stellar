import { Networks } from '@stellar/stellar-sdk/base'
import { describe, expect, it } from 'vitest'

import { bwbSdk, CHAIN, NETWORK, SALE_REFUSAL, type StellarRpc } from '../src/index.js'

const SALE = 'CDN3QJI4KSAULT4I7TQ5PO5SBPDPW3VIUDOAGUFVHUHKLLEZZUYHV4AC'
const rpc: StellarRpc = () => Promise.reject(new Error(`No chain behind ${Networks.TESTNET}`))

describe('bwbSdk', () => {
  it('returns the sale of a Stellar offering when the builder set up Stellar', () => {
    const sale = bwbSdk().withStellar({ rpc, network: NETWORK.TESTNET }).build().sale({ chain: CHAIN.STELLAR, address: SALE })

    expect(sale).toMatchObject({
      success: true,
      data: {
        buy: expect.any(Function),
        authorize: expect.any(Function),
        outcome: expect.any(Function),
        read: expect.any(Function),
      },
    })
  })

  it('refuses a chain the builder did not set up', () => {
    const sale = bwbSdk().build().sale({ chain: CHAIN.STELLAR, address: SALE })

    expect(sale).toMatchObject({ success: false, error: { code: SALE_REFUSAL.CHAIN_NOT_CONFIGURED } })
  })
})

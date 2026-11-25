import { hash, Networks } from '@stellar/stellar-sdk/base'
import { describe, expect, it } from 'vitest'

import { CHAIN_ID, toHex } from '../src/index.js'

describe('CHAIN_ID', () => {
  it('names each Stellar chain by its network id, the SHA-256 of its passphrase', () => {
    expect(CHAIN_ID.STELLAR_TESTNET).toBe(toHex(hash(Networks.TESTNET)))
    expect(CHAIN_ID.STELLAR_MAINNET).toBe(toHex(hash(Networks.PUBLIC)))
  })

  it('names each Base chain by its EVM chain id', () => {
    expect(CHAIN_ID.BASE_TESTNET).toBe(84532)
    expect(CHAIN_ID.BASE_MAINNET).toBe(8453)
  })
})

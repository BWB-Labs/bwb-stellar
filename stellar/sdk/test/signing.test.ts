import { Keypair } from '@stellar/stellar-sdk/base'
import { describe, expect, it, vi } from 'vitest'

import { isSignatureOf, privySigner, toHex } from '../src/signing.js'

const KEY = Keypair.random()
const HASH = 'cd'.repeat(32)

function signatureOf(keypair: Keypair, hash = HASH): string {
  return `0x${toHex(keypair.sign(Buffer.from(hash, 'hex')))}`
}

describe('isSignatureOf', () => {
  it('accepts the signature the address’s key made over the hash', () => {
    expect(isSignatureOf(KEY.publicKey(), HASH, signatureOf(KEY))).toBe(true)
  })

  it('refuses a signature made by another key', () => {
    expect(isSignatureOf(KEY.publicKey(), HASH, signatureOf(Keypair.random()))).toBe(false)
  })

  it('refuses a signature over another hash', () => {
    expect(isSignatureOf(KEY.publicKey(), HASH, signatureOf(KEY, 'ef'.repeat(32)))).toBe(false)
  })

  it('refuses a malformed address or signature', () => {
    expect(isSignatureOf('GNOTANADDRESS', HASH, signatureOf(KEY))).toBe(false)
    expect(isSignatureOf(KEY.publicKey(), HASH, '0x1234')).toBe(false)
  })
})

describe('privySigner', () => {
  it('asks Privy to sign the hash with the wallet and returns the signature bytes', async () => {
    const rawSign = vi.fn(async (_walletId: string, { params }: { params: { hash: string } }) => ({
      signature: `0x${toHex(KEY.sign(Buffer.from(params.hash.slice(2), 'hex')))}`,
    }))

    const signature = await privySigner({ rawSign }, { walletId: 'wallet-1', address: KEY.publicKey() }).sign(Buffer.from(HASH, 'hex'))

    expect(rawSign).toHaveBeenCalledWith('wallet-1', { params: { hash: `0x${HASH}` } })
    expect(KEY.verify(Buffer.from(HASH, 'hex'), Buffer.from(signature))).toBe(true)
  })

  it('throws when Privy answers with something that is not an ed25519 signature', async () => {
    const signer = privySigner({ rawSign: async () => ({ signature: '0xdead' }) }, { walletId: 'wallet-1', address: KEY.publicKey() })

    await expect(signer.sign(Buffer.from(HASH, 'hex'))).rejects.toThrow('no ed25519 signature')
  })
})

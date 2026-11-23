import { Asset, Keypair, Operation, type xdr } from '@stellar/stellar-sdk/base'
import { describe, expect, it, vi } from 'vitest'

import { ensureStellarWallet, walletOpeningOperations, type PrivyUser } from '../src/wallet.js'

const ADDRESS = Keypair.random().publicKey()
const SPONSOR = Keypair.random().publicKey()
const USDC = new Asset('USDC', Keypair.random().publicKey())

function embedded(address: string, chain = 'stellar'): PrivyUser['linked_accounts'][number] {
  return { type: 'wallet', chain_type: chain, address, wallet_client_type: 'privy', connector_type: 'embedded' }
}

function decoded(operation: xdr.Operation) {
  return Operation.fromXDRObject(operation)
}

function users(held: PrivyUser, pregenerated: PrivyUser = held) {
  return { _get: vi.fn(async () => held), pregenerateWallets: vi.fn(async () => pregenerated) }
}

describe('ensureStellarWallet', () => {
  it('returns the Stellar wallet Privy already holds for the user and creates none', async () => {
    const privy = users({ linked_accounts: [embedded('0xabc', 'ethereum'), embedded(ADDRESS)] })

    expect(await ensureStellarWallet(privy, 'did:privy:1')).toMatchObject({ success: true, data: { address: ADDRESS } })
    expect(privy.pregenerateWallets).not.toHaveBeenCalled()
  })

  it('creates a Stellar wallet owned by the user when Privy holds none', async () => {
    const privy = users({ linked_accounts: [] }, { linked_accounts: [embedded(ADDRESS)] })

    expect(await ensureStellarWallet(privy, 'did:privy:1')).toMatchObject({ success: true, data: { address: ADDRESS } })
    expect(privy.pregenerateWallets).toHaveBeenCalledWith('did:privy:1', { wallets: [{ chain_type: 'stellar' }] })
  })

  it('does not take a Stellar wallet the user connected from outside Privy', async () => {
    const external = { ...embedded(ADDRESS), wallet_client_type: 'freighter', connector_type: 'injected' }
    const privy = users({ linked_accounts: [external] }, { linked_accounts: [external] })

    expect(await ensureStellarWallet(privy, 'did:privy:1')).toMatchObject({ success: false, error: { code: 'provisioningRefused' } })
  })

  it('refuses when Privy cannot be reached', async () => {
    const privy = { _get: vi.fn(async () => Promise.reject(new Error('timeout'))), pregenerateWallets: vi.fn() }

    expect(await ensureStellarWallet(privy, 'did:privy:1')).toMatchObject({ success: false, error: { code: 'provisioningRefused' } })
  })
})

describe('walletOpeningOperations', () => {
  it('opens the account with no XLM of its own and its USDC trustline, every reserve sponsored', () => {
    const operations = walletOpeningOperations({ address: ADDRESS, sponsor: SPONSOR, asset: USDC, opensAccount: true }).map(decoded)

    expect(operations).toMatchObject([
      { type: 'beginSponsoringFutureReserves', sponsoredId: ADDRESS, source: SPONSOR },
      { type: 'createAccount', destination: ADDRESS, startingBalance: '0.0000000', source: SPONSOR },
      { type: 'changeTrust', line: { code: 'USDC', issuer: USDC.getIssuer() }, source: ADDRESS },
      { type: 'endSponsoringFutureReserves', source: ADDRESS },
    ])
  })

  it('only adds the trustline under sponsorship when the account already exists', () => {
    const operations = walletOpeningOperations({ address: ADDRESS, sponsor: SPONSOR, asset: USDC, opensAccount: false }).map(decoded)

    expect(operations.map(({ type }) => type)).toEqual(['beginSponsoringFutureReserves', 'changeTrust', 'endSponsoringFutureReserves'])
  })
})

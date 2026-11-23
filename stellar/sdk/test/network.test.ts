import { Networks } from '@stellar/stellar-sdk/base'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { Asset } from '@stellar/stellar-sdk/base'

import { connect, NETWORK, StellarChainMismatchError, usdcOf } from '../src/network.js'

const MANAGED = 'https://managed.example'
const PUBLIC = 'https://public.example'

function stubEndpoints(answers: Record<string, string | null>): Array<string> {
  const hosts: Array<string> = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit) => {
      const host = new URL(url).host
      hosts.push(host)
      const passphrase = answers[host]
      if (passphrase === null) throw new TypeError('fetch failed')
      const { id } = JSON.parse(String(init.body))
      return new Response(JSON.stringify({ jsonrpc: '2.0', id, result: { passphrase, protocolVersion: 23 } }), {
        headers: { 'Content-Type': 'application/json' },
      })
    }),
  )
  return hosts
}

afterEach(() => vi.unstubAllGlobals())

describe('connect', () => {
  it('falls back to the public endpoint when the managed one does not answer', async () => {
    const hosts = stubEndpoints({ 'managed.example': null, 'public.example': Networks.TESTNET })
    const rpc = connect({ network: NETWORK.TESTNET, rpcUrls: [MANAGED, PUBLIC] })

    expect(await rpc(async (_server, passphrase) => passphrase)).toBe(Networks.TESTNET)
    expect(hosts).toEqual(['managed.example', 'public.example'])
  })

  it('refuses an endpoint that serves another network instead of falling through', async () => {
    stubEndpoints({ 'managed.example': Networks.PUBLIC, 'public.example': Networks.TESTNET })
    const rpc = connect({ network: NETWORK.TESTNET, rpcUrls: [MANAGED, PUBLIC] })

    await expect(rpc(async () => 'called')).rejects.toBeInstanceOf(StellarChainMismatchError)
  })

  it('fails naming every endpoint when none answers', async () => {
    stubEndpoints({ 'managed.example': null, 'public.example': null })
    const rpc = connect({ network: NETWORK.TESTNET, rpcUrls: [MANAGED, PUBLIC] })

    await expect(rpc(async () => 'called')).rejects.toThrow('managed.example, public.example')
  })
})

describe('usdcOf', () => {
  it('names the asset whose contract the payment flows use on each network', () => {
    for (const network of [NETWORK.TESTNET, NETWORK.MAINNET]) {
      const { asset, contract } = usdcOf(network)
      expect(asset.contractId(network === NETWORK.TESTNET ? Networks.TESTNET : Networks.PUBLIC)).toBe(contract)
      expect(asset).toBeInstanceOf(Asset)
    }
  })
})

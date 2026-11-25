import type { ChainId } from './chains.js'
import { refuse, succeed, type Result } from './result.js'
import { SALE_REFUSAL, type Sale, type SaleRefusal } from './sale.js'
import { isStellarChainId, type StellarChainId, type StellarRpc } from './stellar/network.js'
import { stellarSale } from './stellar/sale.js'

export type StellarChain = { rpc: StellarRpc }

export type BwbSdkConfig = {
  chains: Partial<Record<StellarChainId, StellarChain>>
}

export type ChainClient = {
  sale(offering: { address: string }): Sale
}

export type BwbSdk = {
  select(chainId: ChainId): Result<ChainClient, SaleRefusal>
  sale(offering: { chainId: ChainId; address: string }): Result<Sale, SaleRefusal>
}

export function createBwbSdk({ chains }: BwbSdkConfig): BwbSdk {
  const select: BwbSdk['select'] = (chainId) => {
    if (!isStellarChainId(chainId)) {
      return refuse({ code: SALE_REFUSAL.CHAIN_NOT_SUPPORTED }, `The SDK has no sale strategy for ${chainId}`)
    }
    const chain = chains[chainId]
    if (chain === undefined) {
      return refuse({ code: SALE_REFUSAL.CHAIN_NOT_CONFIGURED }, `The SDK was not configured for ${chainId}`)
    }
    return succeed({ sale: ({ address }) => stellarSale(chainId, chain.rpc, address) }, `Client for ${chainId}`)
  }

  return {
    select,
    sale({ chainId, address }) {
      const client = select(chainId)
      return client.success ? succeed(client.data.sale({ address }), `Sale ${address} on ${chainId}`) : client
    },
  }
}

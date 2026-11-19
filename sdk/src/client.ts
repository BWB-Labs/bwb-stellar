import type { ChainId } from './chains.js'
import { refuse, succeed, type Result } from './result.js'
import { SALE_REFUSAL, type Sale, type SaleRefusal } from './sale.js'
import { isStellarChainId, type StellarChainId, type StellarRpc } from './stellar/network.js'
import { stellarSale } from './stellar/sale.js'

export type StellarChain = { rpc: StellarRpc }

export type BwbSdkConfig = {
  chains: Partial<Record<StellarChainId, StellarChain>>
}

export type DeployedOffering = {
  chainId: ChainId
  address: string
}

export type BwbSdk = {
  sale(offering: DeployedOffering): Result<Sale, SaleRefusal>
}

export function createBwbSdk({ chains }: BwbSdkConfig): BwbSdk {
  return {
    sale({ chainId, address }) {
      if (!isStellarChainId(chainId)) {
        return refuse({ code: SALE_REFUSAL.CHAIN_NOT_SUPPORTED }, `The SDK has no sale strategy for ${chainId}`)
      }
      const chain = chains[chainId]
      if (chain === undefined) {
        return refuse({ code: SALE_REFUSAL.CHAIN_NOT_CONFIGURED }, `The SDK was not configured for ${chainId}`)
      }
      return succeed(stellarSale(chainId, chain.rpc, address), `Sale ${address} on ${chainId}`)
    },
  }
}

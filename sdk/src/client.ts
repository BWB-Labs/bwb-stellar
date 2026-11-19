import { refuse, succeed, type Result } from './result.js'
import { CHAIN, SALE_REFUSAL, type Chain, type Sale, type SaleRefusal } from './sale.js'
import type { StellarSetup } from './stellar/network.js'
import { stellarSale } from './stellar/sale.js'

export type DeployedOffering = {
  chain: Chain
  address: string
}

export type BwbSdk = {
  sale(offering: DeployedOffering): Result<Sale, SaleRefusal>
}

export type BwbSdkBuilder = {
  withStellar(setup: StellarSetup): BwbSdkBuilder
  build(): BwbSdk
}

type Strategies = Partial<Record<Chain, (address: string) => Sale>>

function builderWith(strategies: Strategies): BwbSdkBuilder {
  return {
    withStellar: (setup) =>
      builderWith({ ...strategies, [CHAIN.STELLAR]: (address: string) => stellarSale(setup, address) }),
    build: () => ({
      sale({ chain, address }) {
        const strategy = strategies[chain]
        if (strategy === undefined) {
          return refuse({ code: SALE_REFUSAL.CHAIN_NOT_CONFIGURED }, `The SDK was not set up for ${chain}`)
        }
        return succeed(strategy(address), `Sale ${address} on ${chain}`)
      },
    }),
  }
}

export function bwbSdk(): BwbSdkBuilder {
  return builderWith({})
}

import { hash, xdr } from '@stellar/stellar-sdk/base'

import { networkIdOf, type Network, type StellarRpc } from './network.js'
import { fromHex, toHex } from './signing.js'
import {
  contractAddressOf,
  contractExists,
  contractSalt,
  createContractOperation,
  invokeContractOperation,
  scv,
  simulateRead,
} from './soroban.js'

const STROOPS_PER_USDC = 10_000_000n
const MIN_SUCCESS_PERCENT = 75
const SALE_ROLE = 'sale'
const SALE_PREPARING = 'Preparing'

export const DEPLOYMENT_STEP = {
  TOKEN: 'token',
  SALE: 'sale',
  GRANT: 'grant',
  ACTIVATION: 'activation',
} as const

export type DeploymentStep = (typeof DEPLOYMENT_STEP)[keyof typeof DEPLOYMENT_STEP]

export const DEPLOYMENT_STEPS: ReadonlyArray<DeploymentStep> = Object.values(DEPLOYMENT_STEP)

export type OfferingRequest = {
  offeringId: string
  treasury: string
  tokenName: string
  tokenSymbol: string
  offeringURI: string
  totalTokens: string
  coolingOffSeconds: string
}

export type OfferingPlatform = {
  network: Network
  platform: string
  platformAdmin: string
  paymentAsset: string
  allowlist: string
  tokenWasmHash: string
  saleWasmHash: string
}

export type OfferingTerms = OfferingRequest &
  Omit<OfferingPlatform, 'network'> & {
    tokenSalt: string
    saleSalt: string
    token: string
    sale: string
  }

export function offeringTerms(request: OfferingRequest, { network, ...platform }: OfferingPlatform): OfferingTerms {
  const networkId = networkIdOf(network)
  const tokenSalt = contractSalt(`bwb:${request.offeringId}:token`)
  const saleSalt = contractSalt(`bwb:${request.offeringId}:sale`)
  return {
    ...request,
    ...platform,
    tokenSalt: toHex(tokenSalt),
    saleSalt: toHex(saleSalt),
    token: contractAddressOf({ deployer: platform.platform, salt: tokenSalt, networkId }),
    sale: contractAddressOf({ deployer: platform.platform, salt: saleSalt, networkId }),
  }
}

export function tokenConstructorArgs(terms: OfferingTerms): Array<xdr.ScVal> {
  return [
    scv.struct({
      admin: scv.address(terms.treasury),
      xfer_admin: scv.address(terms.treasury),
      pauser: scv.address(terms.platformAdmin),
      upgrader: scv.address(terms.platformAdmin),
    }),
    scv.struct({
      name: scv.string(terms.tokenName),
      symbol: scv.string(terms.tokenSymbol),
      offer_uri: scv.string(terms.offeringURI),
    }),
    scv.i128(BigInt(terms.totalTokens)),
    scv.address(terms.sale),
    scv.vec([]),
  ]
}

export function saleConstructorArgs(terms: OfferingTerms): Array<xdr.ScVal> {
  return [
    scv.struct({
      owner: scv.address(terms.treasury),
      pauser: scv.address(terms.platformAdmin),
      upgrader: scv.address(terms.platformAdmin),
    }),
    scv.struct({
      token: scv.address(terms.token),
      payment_asset: scv.address(terms.paymentAsset),
      allowlist: scv.address(terms.allowlist),
    }),
    scv.struct({
      price: scv.i128(STROOPS_PER_USDC),
      cooling_off_secs: scv.u64(BigInt(terms.coolingOffSeconds)),
      min_success_percent: scv.u32(MIN_SUCCESS_PERCENT),
    }),
  ]
}

export function deploymentDigest(network: Network, terms: OfferingTerms): string {
  return toHex(
    hash(
      Buffer.concat([
        networkIdOf(network),
        fromHex(terms.tokenSalt),
        fromHex(terms.saleSalt),
        fromHex(terms.tokenWasmHash),
        fromHex(terms.saleWasmHash),
        scv.vec(tokenConstructorArgs(terms)).toXDR(),
        scv.vec(saleConstructorArgs(terms)).toXDR(),
      ]),
    ),
  )
}

export function deploymentOperation(step: DeploymentStep, terms: OfferingTerms): xdr.Operation {
  if (step === DEPLOYMENT_STEP.TOKEN) {
    return createContractOperation({
      deployer: terms.platform,
      salt: fromHex(terms.tokenSalt),
      wasmHash: fromHex(terms.tokenWasmHash),
      constructorArgs: tokenConstructorArgs(terms),
    })
  }
  if (step === DEPLOYMENT_STEP.SALE) {
    return createContractOperation({
      deployer: terms.platform,
      salt: fromHex(terms.saleSalt),
      wasmHash: fromHex(terms.saleWasmHash),
      constructorArgs: saleConstructorArgs(terms),
    })
  }
  if (step === DEPLOYMENT_STEP.GRANT) {
    return invokeContractOperation({
      contract: terms.allowlist,
      method: 'grant_role',
      args: [scv.address(terms.sale), scv.symbol(SALE_ROLE), scv.address(terms.platform)],
      source: terms.platform,
    })
  }
  return invokeContractOperation({
    contract: terms.sale,
    method: 'activate',
    args: [],
    source: terms.platform,
  })
}

export function isStepApplied(rpc: StellarRpc, step: DeploymentStep, terms: OfferingTerms): Promise<boolean> {
  return rpc(async (server, passphrase) => {
    if (step === DEPLOYMENT_STEP.TOKEN) return contractExists(server, terms.token)
    if (step === DEPLOYMENT_STEP.SALE) return contractExists(server, terms.sale)
    if (step === DEPLOYMENT_STEP.GRANT) {
      const role = await simulateRead(server, passphrase, {
        source: terms.platform,
        operation: invokeContractOperation({
          contract: terms.allowlist,
          method: 'has_role',
          args: [scv.address(terms.sale), scv.symbol(SALE_ROLE)],
          source: terms.platform,
        }),
      })
      return role !== undefined && role !== null
    }
    const state = await simulateRead(server, passphrase, {
      source: terms.platform,
      operation: invokeContractOperation({
        contract: terms.sale,
        method: 'state',
        args: [],
        source: terms.platform,
      }),
    })
    return !String(state).includes(SALE_PREPARING)
  })
}

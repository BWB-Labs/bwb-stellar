import type { StellarChainId } from './stellar/network.js'
import type { Result } from './result.js'

export const SALE_REFUSAL = {
  CHAIN_NOT_CONFIGURED: 'chainNotConfigured',
  CHAIN_NOT_SUPPORTED: 'chainNotSupported',
  CHAIN_MISMATCH: 'chainMismatch',
  CHAIN_UNAVAILABLE: 'chainUnavailable',
  INVALID_BUYER: 'invalidBuyer',
  INVESTOR_NOT_ENABLED: 'investorNotEnabled',
  ALLOCATION_EXCEEDED: 'allocationExceeded',
  TOO_MANY_RECEIPTS: 'tooManyReceipts',
  SALE_NOT_ACTIVE: 'saleNotActive',
  NOT_WHOLE_QUOTAS: 'notWholeQuotas',
  EXCEEDS_SUPPLY: 'exceedsSupply',
  TOO_MANY_PURCHASES: 'tooManyPurchases',
  INSUFFICIENT_BALANCE: 'insufficientBalance',
  BALANCE_FROZEN: 'balanceFrozen',
  TRUSTLINE_MISSING: 'trustlineMissing',
  ENTRY_EXPIRED: 'entryExpired',
  SIGNATURE_MISMATCH: 'signatureMismatch',
  UNEXPECTED_AUTHORIZATION: 'unexpectedAuthorization',
  UNEXPECTED_RESPONSE: 'unexpectedResponse',
  UNREADABLE_RECEIPT: 'unreadableReceipt',
  PURCHASE_NOT_FOUND: 'purchaseNotFound',
  CONTRACT_REFUSED: 'contractRefused',
} as const

export type SaleRefusalCode = (typeof SALE_REFUSAL)[keyof typeof SALE_REFUSAL]

export type SaleRefusal = { code: SaleRefusalCode }

export const SALE_STATE = {
  PREPARING: 'preparing',
  ACTIVE: 'active',
  CLOSING: 'closing',
  SUCCESSFUL: 'successful',
  FAILED: 'failed',
} as const

export type SaleState = (typeof SALE_STATE)[keyof typeof SALE_STATE]

export type PreparedPurchase = {
  chainId: StellarChainId
  buyer: string
  amount: bigint
  entry: string
  entryHash: string
  expirationLedger: number
}

export type AuthorizedPurchase = {
  chainId: StellarChainId
  invocation: string
}

export type SaleReceipt = {
  chainId: StellarChainId
  resultMeta: string
}

export type PurchaseOutcome = {
  quotas: bigint
  paid: bigint
}

export type SaleView = {
  state: SaleState
  pricePerQuota: bigint
  supply: bigint
  soldQuotas: bigint
  raised: bigint
  paymentAsset: string
}

export type Sale = {
  buy(purchase: { buyer: string; amount: bigint }): Promise<Result<PreparedPurchase, SaleRefusal>>
  authorize(signed: { prepared: PreparedPurchase; signature: string }): Promise<Result<AuthorizedPurchase, SaleRefusal>>
  outcome(settled: { buyer: string; receipt: SaleReceipt }): Result<PurchaseOutcome, SaleRefusal>
  read(): Promise<Result<SaleView, SaleRefusal>>
}

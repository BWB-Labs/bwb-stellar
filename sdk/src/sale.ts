import type { Result } from './result.js'

export const CHAIN = {
  STELLAR: 'stellar',
} as const

export type Chain = (typeof CHAIN)[keyof typeof CHAIN]

export const SALE_REFUSAL = {
  CHAIN_NOT_CONFIGURED: 'chainNotConfigured',
  CHAIN_UNAVAILABLE: 'chainUnavailable',
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
  chain: typeof CHAIN.STELLAR
  buyer: string
  amount: bigint
  entry: string
  entryHash: string
  expirationLedger: number
}

export type AuthorizedPurchase = {
  chain: typeof CHAIN.STELLAR
  invocation: string
}

export type SaleReceipt = {
  chain: typeof CHAIN.STELLAR
  resultMeta: string
}

export type PurchaseOutcome = {
  buyer: string
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
